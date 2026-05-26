#!/usr/bin/env python3
"""Safe supplier-agent bridge for wx-xd purchase tasks.

The bridge intentionally does not automate supplier-platform login or order
placement. It only exports non-sensitive purchase tasks and applies explicit
agent/operator results through the authenticated wx-xd local API.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timezone, timedelta
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from wx_xd_local_api import (  # noqa: E402
    API_KEY_ENV,
    BASE_URL_ENV,
    DEFAULT_BASE_URL,
    LocalApiError,
    call_api,
    drop_empty,
    print_json,
    require_api_key,
)

SHANGHAI_TZ = timezone(timedelta(hours=8), "Asia/Shanghai")
DEFAULT_EXPORT_DIR = Path("artifacts/supplier-agent")

SAFE_TASK_FIELDS = [
    "id",
    "wechat_order_id",
    "shop_id",
    "shop_name",
    "status",
    "external_product_id",
    "source_url",
    "external_sku_id",
    "title",
    "quantity",
    "estimated_revenue",
    "estimated_cost",
    "estimated_profit",
    "supplier_name",
    "supplier_product_id",
    "error_summary",
    "created_at",
    "updated_at",
]

FORBIDDEN_KEY_FRAGMENTS = [
    "access_token",
    "address",
    "apikey",
    "api_key",
    "app_secret",
    "cookie",
    "mobile",
    "openid",
    "password",
    "phone",
    "recipient",
    "receiver",
    "session",
    "tel",
]

ISSUE_TYPES = {
    "out_of_stock",
    "price_changed",
    "supplier_cancelled",
    "quality_risk",
    "other",
}

ACTION_FIELDS = {
    "shipment": {
        "purchase_task_id",
        "action",
        "delivery_id",
        "delivery_name",
        "waybill_id",
        "deliver_type",
        "estimated_cost",
    },
    "issue": {"purchase_task_id", "action", "issue_type", "note"},
    "mapping": {
        "purchase_task_id",
        "action",
        "external_product_id",
        "external_sku_id",
        "source_url",
        "supplier_name",
        "supplier_product_id",
        "estimated_cost",
        "note",
    },
}


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    try:
        payload = dispatch(args)
    except LocalApiError as error:
        print(error, file=sys.stderr)
        return 1
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 2
    print_json(payload)
    if isinstance(payload, dict) and payload.get("failed", 0) > 0:
        return 1
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Export non-sensitive purchase tasks and apply supplier-agent "
            "results through the wx-xd local API."
        )
    )
    parser.add_argument(
        "--base-url",
        default=os.environ.get(BASE_URL_ENV, DEFAULT_BASE_URL),
        help=f"Local API base URL. Default: {DEFAULT_BASE_URL}",
    )
    parser.add_argument(
        "--api-key",
        default=os.environ.get(API_KEY_ENV),
        help=f"API key. Prefer {API_KEY_ENV} environment variable.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    export_parser = subparsers.add_parser(
        "export", help="Export non-sensitive purchase tasks for an external agent."
    )
    export_parser.add_argument("--status", default="pending_purchase")
    export_parser.add_argument("--limit", type=int, default=100)
    export_parser.add_argument(
        "--format", choices=["jsonl", "json", "md"], default="jsonl"
    )
    export_parser.add_argument(
        "--out",
        help="Output file. Defaults to artifacts/supplier-agent with a Shanghai timestamp.",
    )

    apply_parser = subparsers.add_parser(
        "apply", help="Apply shipment, issue, or mapping results from JSON/JSONL."
    )
    apply_parser.add_argument(
        "--json-file", required=True, help="Read result records from JSON/JSONL. Use '-' for stdin."
    )
    apply_parser.add_argument(
        "--dry-run", action="store_true", help="Validate records without calling wx-xd."
    )
    apply_parser.add_argument(
        "--continue-on-error",
        action="store_true",
        help="Continue applying later records after one record fails.",
    )
    apply_parser.add_argument(
        "--result-out", help="Optional file path for the apply result summary."
    )

    template_parser = subparsers.add_parser(
        "template", help="Write a JSONL result template for supplier agents/operators."
    )
    template_parser.add_argument(
        "--out",
        help="Output template file. Defaults to artifacts/supplier-agent/supplier-results-template.jsonl.",
    )
    return parser


def dispatch(args: argparse.Namespace) -> Any:
    if args.command == "export":
        return export_purchase_tasks(args)
    if args.command == "apply":
        return apply_results(args)
    if args.command == "template":
        return write_template(args)
    raise ValueError(f"Unsupported command: {args.command}")


def export_purchase_tasks(args: argparse.Namespace) -> dict[str, Any]:
    query = {"limit": args.limit}
    if args.status and args.status != "all":
        query["status"] = args.status
    payload = call_api(
        args.base_url,
        "GET",
        "/api/purchase-tasks",
        query=query,
        api_key=require_api_key(args.api_key),
    )
    tasks = [sanitize_task(item) for item in payload.get("items", [])]
    output_path = Path(args.out) if args.out else default_export_path(args.format)
    write_export(output_path, args.format, tasks)
    return {
        "file_path": str(output_path),
        "format": args.format,
        "status": args.status,
        "exported_count": len(tasks),
        "total": payload.get("total", len(tasks)),
        "sensitive_fields": "excluded",
    }


def apply_results(args: argparse.Namespace) -> dict[str, Any]:
    records = load_result_records(args.json_file)
    results: list[dict[str, Any]] = []
    api_key = None if args.dry_run else require_api_key(args.api_key)

    for index, raw_record in enumerate(records, start=1):
        try:
            action, purchase_task_id, path, body = build_apply_request(raw_record)
            if args.dry_run:
                results.append(
                    {
                        "index": index,
                        "purchase_task_id": purchase_task_id,
                        "action": action,
                        "status": "dry_run",
                    }
                )
                continue
            response = call_api(
                args.base_url,
                "POST",
                path,
                body=body,
                api_key=api_key,
            )
            results.append(
                {
                    "index": index,
                    "purchase_task_id": purchase_task_id,
                    "action": action,
                    "status": "success",
                    "response": response,
                }
            )
        except (LocalApiError, ValueError) as error:
            results.append(
                {
                    "index": index,
                    "purchase_task_id": raw_record.get("purchase_task_id"),
                    "action": raw_record.get("action"),
                    "status": "failed",
                    "error": str(error),
                }
            )
            if not args.continue_on_error:
                break

    summary = {
        "dry_run": args.dry_run,
        "processed": len(results),
        "succeeded": sum(1 for item in results if item["status"] in {"success", "dry_run"}),
        "failed": sum(1 for item in results if item["status"] == "failed"),
        "results": results,
    }
    if args.result_out:
        write_json(Path(args.result_out), summary)
        summary["result_file_path"] = args.result_out
    return summary


def write_template(args: argparse.Namespace) -> dict[str, Any]:
    output_path = (
        Path(args.out)
        if args.out
        else DEFAULT_EXPORT_DIR / "supplier-results-template.jsonl"
    )
    examples = [
        {
            "purchase_task_id": "purchase-task-id",
            "action": "shipment",
            "delivery_id": "SF",
            "delivery_name": "顺丰速运",
            "waybill_id": "SF1234567890",
            "deliver_type": 1,
            "estimated_cost": 18.8,
        },
        {
            "purchase_task_id": "purchase-task-id",
            "action": "issue",
            "issue_type": "out_of_stock",
            "note": "supplier reported no stock",
        },
        {
            "purchase_task_id": "purchase-task-id",
            "action": "mapping",
            "external_product_id": "external-product-id",
            "external_sku_id": "external-sku-id",
            "source_url": "https://example.com/source-product",
            "supplier_name": "supplier",
            "supplier_product_id": "supplier-product-id",
            "estimated_cost": 18.8,
            "note": "operator confirmed mapping",
        },
    ]
    write_jsonl(output_path, examples)
    return {
        "file_path": str(output_path),
        "record_count": len(examples),
        "note": "Template contains placeholders only; do not add recipient or credential fields.",
    }


def sanitize_task(item: dict[str, Any]) -> dict[str, Any]:
    task = {field: item.get(field) for field in SAFE_TASK_FIELDS if field in item}
    task["purchase_task_id"] = task.pop("id", item.get("id"))
    task["needs_mapping"] = not bool(task.get("external_product_id") and task.get("external_sku_id"))
    task["allowed_result_actions"] = ["shipment", "issue", "mapping"]
    return task


def build_apply_request(record: dict[str, Any]) -> tuple[str, str, str, dict[str, Any]]:
    if not isinstance(record, dict):
        raise ValueError("每条结果记录必须是 JSON object。")
    forbidden_paths = find_forbidden_keys(record)
    if forbidden_paths:
        raise ValueError(
            "结果记录包含禁止字段："
            + ", ".join(forbidden_paths)
            + "；供应商 agent 不得传递收件信息、密钥、Cookie 或平台登录凭证。"
        )

    action = str(record.get("action", "")).strip()
    if action not in ACTION_FIELDS:
        raise ValueError("action 必须是 shipment/issue/mapping。")
    unknown_fields = set(record) - ACTION_FIELDS[action]
    if unknown_fields:
        raise ValueError(
            f"{action} 结果包含未知字段：{', '.join(sorted(unknown_fields))}。"
        )
    purchase_task_id = str(record.get("purchase_task_id", "")).strip()
    if not purchase_task_id:
        raise ValueError("purchase_task_id 不能为空。")

    if action == "shipment":
        body = drop_empty(
            {
                "delivery_id": record.get("delivery_id"),
                "delivery_name": record.get("delivery_name"),
                "waybill_id": record.get("waybill_id"),
                "deliver_type": record.get("deliver_type", 1),
                "estimated_cost": record.get("estimated_cost"),
            }
        )
        return action, purchase_task_id, f"/api/purchase-tasks/{purchase_task_id}/shipment", body

    if action == "issue":
        issue_type = str(record.get("issue_type", "")).strip()
        if issue_type not in ISSUE_TYPES:
            raise ValueError(
                "issue_type 必须是 out_of_stock/price_changed/supplier_cancelled/quality_risk/other。"
            )
        body = drop_empty({"issue_type": issue_type, "note": record.get("note")})
        return action, purchase_task_id, f"/api/purchase-tasks/{purchase_task_id}/issue", body

    body = drop_empty(
        {
            "external_product_id": record.get("external_product_id"),
            "external_sku_id": record.get("external_sku_id"),
            "source_url": record.get("source_url"),
            "supplier_name": record.get("supplier_name"),
            "supplier_product_id": record.get("supplier_product_id"),
            "estimated_cost": record.get("estimated_cost"),
            "note": record.get("note"),
        }
    )
    return action, purchase_task_id, f"/api/purchase-tasks/{purchase_task_id}/mapping", body


def load_result_records(path: str) -> list[dict[str, Any]]:
    raw = sys.stdin.read() if path == "-" else Path(path).read_text(encoding="utf-8")
    stripped = raw.strip()
    if not stripped:
        return []
    if stripped.startswith("[") or stripped.startswith("{"):
        try:
            payload = json.loads(stripped)
        except json.JSONDecodeError:
            payload = None
        if payload is not None:
            if isinstance(payload, list):
                return payload
            if isinstance(payload, dict):
                if isinstance(payload.get("items"), list):
                    return payload["items"]
                if isinstance(payload.get("results"), list):
                    return payload["results"]
                if "action" in payload and "purchase_task_id" in payload:
                    return [payload]
            raise ValueError("JSON 输入必须是数组，或包含 items/results 数组。")
    records = []
    for line_no, line in enumerate(raw.splitlines(), start=1):
        line = line.strip()
        if not line:
            continue
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError as error:
            raise ValueError(f"JSONL 第 {line_no} 行解析失败：{error}") from error
    return records


def find_forbidden_keys(value: Any, prefix: str = "$") -> list[str]:
    paths: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            normalized = normalize_key(str(key))
            if any(fragment in normalized for fragment in FORBIDDEN_KEY_FRAGMENTS):
                paths.append(f"{prefix}.{key}")
            paths.extend(find_forbidden_keys(child, f"{prefix}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            paths.extend(find_forbidden_keys(child, f"{prefix}[{index}]"))
    return paths


def normalize_key(key: str) -> str:
    return key.lower().replace("-", "_").replace(" ", "_")


def default_export_path(format_name: str) -> Path:
    stamp = datetime.now(SHANGHAI_TZ).strftime("%Y%m%d-%H%M%S")
    suffix = "md" if format_name == "md" else format_name
    return DEFAULT_EXPORT_DIR / f"purchase-tasks-{stamp}.{suffix}"


def write_export(path: Path, format_name: str, tasks: list[dict[str, Any]]) -> None:
    if format_name == "jsonl":
        write_jsonl(path, tasks)
    elif format_name == "json":
        write_json(path, tasks)
    elif format_name == "md":
        write_markdown(path, tasks)
    else:
        raise ValueError(f"Unsupported export format: {format_name}")


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = "".join(
        json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n"
        for record in records
    )
    path.write_text(content, encoding="utf-8")


def write_markdown(path: Path, tasks: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# 供应商采购任务清单",
        "",
        "本文件只包含非敏采购字段，不包含收件人姓名、手机号、地址、密钥或供应商平台凭证。货源链接用于人工采购定位，不得包含登录态、Cookie、token 或一次性私密参数。",
        "",
        "| 采购任务 ID | 状态 | 店铺 | 微信订单号 | 外部商品 ID | 货源链接 | 外部 SKU | 商品 | 数量 | 预估成本 | 异常 |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | ---: | ---: | --- |",
    ]
    for task in tasks:
        lines.append(
            "| "
            + " | ".join(
                markdown_cell(value)
                for value in [
                    task.get("purchase_task_id"),
                    task.get("status"),
                    task.get("shop_name"),
                    task.get("wechat_order_id"),
                    task.get("external_product_id"),
                    task.get("source_url"),
                    task.get("external_sku_id"),
                    task.get("title"),
                    task.get("quantity"),
                    task.get("estimated_cost"),
                    task.get("error_summary"),
                ]
            )
            + " |"
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def markdown_cell(value: Any) -> str:
    if value is None:
        return ""
    return str(value).replace("|", "\\|").replace("\n", " ")


if __name__ == "__main__":
    raise SystemExit(main())
