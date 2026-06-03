#!/usr/bin/env python3
"""CLI helper for the wx-xd local HTTP API.

This script is intentionally dependency-free so an external agent/skill can
reuse it directly. Keep secrets in environment variables or process args; the
script never prints the API key.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


DEFAULT_BASE_URL = "http://127.0.0.1:17890"
API_KEY_ENV = "WX_XD_API_KEY"
BASE_URL_ENV = "WX_XD_BASE_URL"
RUNNER_PATHS = {
    "aftersale-sync": "/api/runners/aftersale-sync",
    "delivery-submit": "/api/runners/delivery-submit",
    "guarantee-sync": "/api/runners/guarantee-sync",
    "inventory-scan": "/api/runners/inventory-risk-scan",
    "operations": "/api/runners/operations",
    "order-detail-sync": "/api/runners/order-detail-sync",
    "order-sync": "/api/runners/order-sync",
    "price-confirm": "/api/runners/price-confirm",
    "price-precheck": "/api/runners/price-precheck",
    "price-submit": "/api/runners/price-submit",
    "purchase-task-generation": "/api/runners/purchase-task-generation",
    "publish-pipeline": "/api/runners/publish-pipeline",
}


class LocalApiError(RuntimeError):
    def __init__(self, status: int | None, payload: Any):
        self.status = status
        self.payload = payload
        super().__init__(format_error(status, payload))


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
    return 0


def add_json_payload_args(parser: argparse.ArgumentParser) -> None:
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--json-file",
        help="Read request payload from a JSON file. Use '-' to read stdin.",
    )
    group.add_argument(
        "--json",
        help="Inline request payload JSON string.",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Call wx-xd local API for publish, price, purchase, and operations automation."
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

    subparsers.add_parser("health", help="Check local API health.")

    subparsers.add_parser("shop-groups", help="List shop groups for external targeting.")

    publish_create_parser = subparsers.add_parser(
        "publish-create", help="Create one external publish job from a JSON payload."
    )
    add_json_payload_args(publish_create_parser)

    publish_get_parser = subparsers.add_parser(
        "publish-get", help="Get one publish job by task id."
    )
    publish_get_parser.add_argument("task_id")

    price_create_parser = subparsers.add_parser(
        "price-create", help="Create one batch price update job from a JSON payload."
    )
    add_json_payload_args(price_create_parser)

    price_get_parser = subparsers.add_parser(
        "price-get", help="Get one price update job by task id."
    )
    price_get_parser.add_argument("task_id")

    subparsers.add_parser("task-runs", help="List local task runs.")
    subparsers.add_parser("notifications", help="List unread notifications.")

    subparsers.add_parser("delivery-settings", help="Get delivery automation settings.")

    auto_send_parser = subparsers.add_parser(
        "delivery-auto-send", help="Enable or disable automatic WeChat delivery submission."
    )
    auto_send_group = auto_send_parser.add_mutually_exclusive_group(required=True)
    auto_send_group.add_argument("--enabled", action="store_true")
    auto_send_group.add_argument("--disabled", action="store_true")

    delivery_companies_parser = subparsers.add_parser(
        "delivery-companies", help="List cached WeChat delivery companies."
    )
    delivery_companies_parser.add_argument("--shop-id")

    delivery_companies_sync_parser = subparsers.add_parser(
        "delivery-companies-sync", help="Sync WeChat delivery companies for one shop."
    )
    delivery_companies_sync_parser.add_argument("shop_id")
    delivery_companies_sync_parser.add_argument("--ewaybill-only", action="store_true")

    delivery_shipments_parser = subparsers.add_parser(
        "delivery-shipments", help="List delivery shipment queue items."
    )
    delivery_shipments_parser.add_argument("--status")
    delivery_shipments_parser.add_argument("--limit", type=int, default=100, help="1-500, default 100.")

    delivery_record_parser = subparsers.add_parser(
        "delivery-record", help="Record order-level shipment information."
    )
    delivery_record_parser.add_argument("--order-id")
    delivery_record_parser.add_argument("--shop-id")
    delivery_record_parser.add_argument("--wechat-order-id")
    delivery_record_parser.add_argument("--delivery-id")
    delivery_record_parser.add_argument("--delivery-name")
    delivery_record_parser.add_argument("--waybill-id")
    delivery_record_parser.add_argument("--deliver-type", type=int, default=1)

    delivery_retry_parser = subparsers.add_parser(
        "delivery-retry", help="Retry one failed or pending delivery shipment."
    )
    delivery_retry_parser.add_argument("shipment_id")

    runner_parser = subparsers.add_parser(
        "runner", help="Trigger one local queue runner by name."
    )
    runner_parser.add_argument(
        "name",
        choices=sorted(RUNNER_PATHS),
        help="Runner name, e.g. publish-pipeline, price-submit, operations.",
    )

    list_parser = subparsers.add_parser(
        "purchase-tasks", help="List non-sensitive purchase tasks."
    )
    list_parser.add_argument("--status", help="Optional purchase task status filter.")
    list_parser.add_argument("--limit", type=int, default=100, help="1-500, default 100.")

    mapping_parser = subparsers.add_parser(
        "purchase-mapping", help="Resolve external product/SKU mapping for one purchase task."
    )
    mapping_parser.add_argument("purchase_task_id")
    mapping_parser.add_argument("--external-product-id", required=True)
    mapping_parser.add_argument("--external-sku-id", required=True)
    mapping_parser.add_argument("--source-url")
    mapping_parser.add_argument("--supplier-name")
    mapping_parser.add_argument("--supplier-product-id")
    mapping_parser.add_argument("--estimated-cost", type=float)
    mapping_parser.add_argument("--note")

    inventory_parser = subparsers.add_parser(
        "inventory-risks", help="List local inventory risk signals."
    )
    inventory_parser.add_argument("--status", help="Optional inventory risk status filter.")
    inventory_parser.add_argument("--limit", type=int, default=200, help="1-500, default 200.")

    sales_parser = subparsers.add_parser(
        "product-sales", help="List product-level sales and operation analysis."
    )
    sales_parser.add_argument("--status", help="Optional operation status filter.")
    sales_parser.add_argument("--limit", type=int, default=200, help="1-500, default 200.")

    subparsers.add_parser(
        "inventory-scan", help="Run one inventory risk scan and create notifications."
    )

    guarantee_parser = subparsers.add_parser(
        "guarantee-orders", help="List cached WeChat guarantee/dispute orders."
    )
    guarantee_parser.add_argument("--status", help="Optional guarantee status filter.")
    guarantee_parser.add_argument("--limit", type=int, default=200, help="1-500, default 200.")

    subparsers.add_parser(
        "guarantee-sync", help="Run one WeChat guarantee/dispute order sync."
    )

    guarantee_followup_parser = subparsers.add_parser(
        "guarantee-followup",
        help="Record local follow-up status and optional supplier compensation for one guarantee order.",
    )
    guarantee_followup_parser.add_argument("guarantee_order_id")
    guarantee_followup_parser.add_argument(
        "--status",
        required=True,
        choices=[
            "pending",
            "in_progress",
            "waiting_supplier",
            "evidence_ready",
            "resolved",
            "ignored",
        ],
        help="Local follow-up status.",
    )
    guarantee_followup_parser.add_argument(
        "--party", choices=["supplier", "merchant", "customer", "platform", "unknown"]
    )
    guarantee_followup_parser.add_argument("--note")
    guarantee_followup_parser.add_argument("--supplier-compensation-cents", type=int)

    aftersales_parser = subparsers.add_parser(
        "aftersales", help="List cached aftersale exception records."
    )
    aftersales_parser.add_argument("--status", help="Optional aftersale status filter.")
    aftersales_parser.add_argument("--limit", type=int, default=200, help="1-500, default 200.")

    evidence_parser = subparsers.add_parser(
        "aftersale-evidence", help="List local aftersale/guarantee evidence records."
    )
    evidence_parser.add_argument("--target-type", choices=["aftersale", "guarantee"])
    evidence_parser.add_argument("--target-id")
    evidence_parser.add_argument("--status", choices=["draft", "ready", "used", "archived"])
    evidence_parser.add_argument("--limit", type=int, default=200, help="1-500, default 200.")

    evidence_record_parser = subparsers.add_parser(
        "aftersale-evidence-record",
        help="Record local evidence metadata without uploading to WeChat.",
    )
    evidence_record_parser.add_argument(
        "--target-type", required=True, choices=["aftersale", "guarantee"]
    )
    evidence_record_parser.add_argument("--target-id", required=True)
    evidence_record_parser.add_argument(
        "--evidence-type",
        required=True,
        choices=[
            "image",
            "text",
            "chat_record",
            "logistics",
            "supplier_proof",
            "quality_check",
            "other",
        ],
    )
    evidence_record_parser.add_argument("--title", required=True)
    evidence_record_parser.add_argument("--content-text")
    evidence_record_parser.add_argument("--local-file-path")
    evidence_record_parser.add_argument("--source-url")
    evidence_record_parser.add_argument(
        "--status", choices=["draft", "ready", "used", "archived"]
    )

    evidence_status_parser = subparsers.add_parser(
        "aftersale-evidence-status",
        help="Update local evidence status without uploading to WeChat.",
    )
    evidence_status_parser.add_argument("evidence_id")
    evidence_status_parser.add_argument(
        "--status", required=True, choices=["draft", "ready", "used", "archived"]
    )

    evidence_export_parser = subparsers.add_parser(
        "aftersale-evidence-export",
        help="Export local evidence metadata package without file contents.",
    )
    evidence_export_parser.add_argument("--target-type", choices=["aftersale", "guarantee"])
    evidence_export_parser.add_argument("--target-id")
    evidence_export_parser.add_argument("--status", choices=["draft", "ready", "used", "archived"])
    evidence_export_parser.add_argument("--format", choices=["jsonl", "json", "md"], default="md")

    supplier_followups_parser = subparsers.add_parser(
        "supplier-aftersale-followups",
        help="List local supplier aftersale/guarantee collaboration records.",
    )
    supplier_followups_parser.add_argument("--target-type", choices=["aftersale", "guarantee"])
    supplier_followups_parser.add_argument("--target-id")
    supplier_followups_parser.add_argument(
        "--status",
        choices=[
            "pending",
            "contacted",
            "waiting_supplier",
            "evidence_ready",
            "compensation_pending",
            "closed",
        ],
    )
    supplier_followups_parser.add_argument("--limit", type=int, default=200, help="1-500, default 200.")

    supplier_followup_record_parser = subparsers.add_parser(
        "supplier-aftersale-followup-record",
        help="Record local supplier aftersale/guarantee collaboration without platform calls.",
    )
    supplier_followup_record_parser.add_argument(
        "--target-type", required=True, choices=["aftersale", "guarantee"]
    )
    supplier_followup_record_parser.add_argument("--target-id", required=True)
    supplier_followup_record_parser.add_argument(
        "--followup-type",
        required=True,
        choices=[
            "contact",
            "evidence_request",
            "evidence_received",
            "compensation",
            "return_refund",
            "other",
        ],
    )
    supplier_followup_record_parser.add_argument(
        "--status",
        required=True,
        choices=[
            "pending",
            "contacted",
            "waiting_supplier",
            "evidence_ready",
            "compensation_pending",
            "closed",
        ],
    )
    supplier_followup_record_parser.add_argument("--note", required=True)
    supplier_followup_record_parser.add_argument("--purchase-task-id")
    supplier_followup_record_parser.add_argument("--supplier-name")

    responsibility_parser = subparsers.add_parser(
        "aftersale-responsibility",
        help="Record responsibility and optional supplier compensation for one aftersale order.",
    )
    responsibility_parser.add_argument("aftersale_id")
    responsibility_parser.add_argument(
        "--party",
        required=True,
        choices=["supplier", "merchant", "customer", "platform", "unknown"],
    )
    responsibility_parser.add_argument("--note")
    responsibility_parser.add_argument("--supplier-compensation-cents", type=int)

    aftersale_accept_parser = subparsers.add_parser(
        "aftersale-accept", help="Manually submit accept action for one aftersale order."
    )
    aftersale_accept_parser.add_argument("aftersale_id")
    aftersale_accept_parser.add_argument("--address-id")
    aftersale_accept_parser.add_argument("--accept-type", type=int, choices=[1, 2])
    aftersale_accept_parser.add_argument("--note")

    aftersale_reject_parser = subparsers.add_parser(
        "aftersale-reject", help="Manually submit reject action for one aftersale order."
    )
    aftersale_reject_parser.add_argument("aftersale_id")
    aftersale_reject_parser.add_argument("--reject-reason-type", type=int, required=True)
    aftersale_reject_parser.add_argument("--reject-reason")
    aftersale_reject_parser.add_argument("--note")

    reject_reasons_parser = subparsers.add_parser(
        "aftersale-reject-reasons", help="List cached WeChat aftersale reject reasons."
    )
    reject_reasons_parser.add_argument("--shop-id")
    reject_reasons_parser.add_argument("--reject-scene", type=int)

    reject_reasons_sync_parser = subparsers.add_parser(
        "aftersale-reject-reasons-sync",
        help="Sync WeChat aftersale reject reasons for one shop.",
    )
    reject_reasons_sync_parser.add_argument("shop_id")

    profits_parser = subparsers.add_parser(
        "order-profits", help="List order profit summaries."
    )
    profits_parser.add_argument("--status", help="Optional order/profit status filter.")
    profits_parser.add_argument("--limit", type=int, default=200, help="1-500, default 200.")

    profit_adjustment_parser = subparsers.add_parser(
        "order-profit-adjustment",
        help="Record one order-level profit adjustment.",
    )
    profit_adjustment_parser.add_argument("order_id")
    profit_adjustment_parser.add_argument(
        "--kind",
        required=True,
        choices=[
            "purchase_freight",
            "refund",
            "aftersale_compensation",
            "other_cost",
            "other_income",
        ],
    )
    profit_adjustment_parser.add_argument("--amount-cents", type=int, required=True)
    profit_adjustment_parser.add_argument("--note")

    shipment_parser = subparsers.add_parser(
        "purchase-shipment", help="Record supplier shipment for one purchase task."
    )
    shipment_parser.add_argument("purchase_task_id")
    shipment_parser.add_argument("--delivery-id")
    shipment_parser.add_argument("--delivery-name")
    shipment_parser.add_argument("--waybill-id")
    shipment_parser.add_argument("--deliver-type", type=int, default=1)
    shipment_parser.add_argument("--estimated-cost", type=float)

    issue_parser = subparsers.add_parser(
        "purchase-issue", help="Mark one purchase task as a supplier issue."
    )
    issue_parser.add_argument("purchase_task_id")
    issue_parser.add_argument(
        "--issue-type",
        required=True,
        choices=[
            "out_of_stock",
            "price_changed",
            "supplier_cancelled",
            "quality_risk",
            "other",
        ],
    )
    issue_parser.add_argument("--note")
    return parser


def dispatch(args: argparse.Namespace) -> Any:
    if args.command == "health":
        return call_api(args.base_url, "GET", "/health", api_key=None, auth=False)
    if args.command == "shop-groups":
        return call_api(
            args.base_url,
            "GET",
            "/api/shop-groups",
            api_key=require_api_key(args.api_key),
        )
    if args.command == "publish-create":
        return call_api(
            args.base_url,
            "POST",
            "/api/publish-jobs",
            body=load_json_payload(args),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "publish-get":
        return call_api(
            args.base_url,
            "GET",
            f"/api/publish-jobs/{args.task_id}",
            api_key=require_api_key(args.api_key),
        )
    if args.command == "price-create":
        return call_api(
            args.base_url,
            "POST",
            "/api/price-update-jobs",
            body=load_json_payload(args),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "price-get":
        return call_api(
            args.base_url,
            "GET",
            f"/api/price-update-jobs/{args.task_id}",
            api_key=require_api_key(args.api_key),
        )
    if args.command == "task-runs":
        return call_api(
            args.base_url,
            "GET",
            "/api/task-runs",
            api_key=require_api_key(args.api_key),
        )
    if args.command == "notifications":
        return call_api(
            args.base_url,
            "GET",
            "/api/notifications",
            api_key=require_api_key(args.api_key),
        )
    if args.command == "delivery-settings":
        return call_api(
            args.base_url,
            "GET",
            "/api/delivery-settings",
            api_key=require_api_key(args.api_key),
        )
    if args.command == "delivery-auto-send":
        return call_api(
            args.base_url,
            "POST",
            "/api/delivery-settings/auto-send",
            body={"enabled": bool(args.enabled)},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "delivery-companies":
        return call_api(
            args.base_url,
            "GET",
            "/api/delivery-companies",
            query=drop_empty({"shop_id": args.shop_id}),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "delivery-companies-sync":
        return call_api(
            args.base_url,
            "POST",
            "/api/delivery-companies/sync",
            body={"shop_id": args.shop_id, "ewaybill_only": args.ewaybill_only},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "delivery-shipments":
        return call_api(
            args.base_url,
            "GET",
            "/api/delivery-shipments",
            query=drop_empty({"status": args.status, "limit": args.limit}),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "delivery-record":
        body = {
            "order_id": args.order_id,
            "shop_id": args.shop_id,
            "wechat_order_id": args.wechat_order_id,
            "delivery_id": args.delivery_id,
            "delivery_name": args.delivery_name,
            "waybill_id": args.waybill_id,
            "deliver_type": args.deliver_type,
        }
        return call_api(
            args.base_url,
            "POST",
            "/api/delivery-shipments",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "delivery-retry":
        return call_api(
            args.base_url,
            "POST",
            f"/api/delivery-shipments/{args.shipment_id}/retry",
            body={},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "runner":
        return call_api(
            args.base_url,
            "POST",
            RUNNER_PATHS[args.name],
            body={},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "purchase-tasks":
        query = {
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/purchase-tasks",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "purchase-mapping":
        body = {
            "external_product_id": args.external_product_id,
            "external_sku_id": args.external_sku_id,
            "source_url": args.source_url,
            "supplier_name": args.supplier_name,
            "supplier_product_id": args.supplier_product_id,
            "estimated_cost": args.estimated_cost,
            "note": args.note,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/purchase-tasks/{args.purchase_task_id}/mapping",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "inventory-risks":
        query = {
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/inventory-risks",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "product-sales":
        query = {
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/product-sales-analysis",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "inventory-scan":
        return call_api(
            args.base_url,
            "POST",
            "/api/runners/inventory-risk-scan",
            body={},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "guarantee-orders":
        query = {
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/guarantee-orders",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "guarantee-sync":
        return call_api(
            args.base_url,
            "POST",
            "/api/runners/guarantee-sync",
            body={},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "guarantee-followup":
        body = {
            "handling_status": args.status,
            "responsibility_party": args.party,
            "handling_note": args.note,
            "supplier_compensation_cents": args.supplier_compensation_cents,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/guarantee-orders/{args.guarantee_order_id}/followup",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersales":
        query = {
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/aftersales",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-evidence":
        query = {
            "target_type": args.target_type,
            "target_id": args.target_id,
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/aftersale-evidence",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-evidence-record":
        body = {
            "target_type": args.target_type,
            "target_id": args.target_id,
            "evidence_type": args.evidence_type,
            "title": args.title,
            "content_text": args.content_text,
            "local_file_path": args.local_file_path,
            "source_url": args.source_url,
            "status": args.status,
        }
        return call_api(
            args.base_url,
            "POST",
            "/api/aftersale-evidence",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-evidence-status":
        return call_api(
            args.base_url,
            "POST",
            f"/api/aftersale-evidence/{args.evidence_id}/status",
            body={"status": args.status},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-evidence-export":
        body = {
            "target_type": args.target_type,
            "target_id": args.target_id,
            "status": args.status,
            "format": args.format,
        }
        return call_api(
            args.base_url,
            "POST",
            "/api/aftersale-evidence/export",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "supplier-aftersale-followups":
        query = {
            "target_type": args.target_type,
            "target_id": args.target_id,
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/supplier-aftersale-followups",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "supplier-aftersale-followup-record":
        body = {
            "target_type": args.target_type,
            "target_id": args.target_id,
            "followup_type": args.followup_type,
            "status": args.status,
            "note": args.note,
            "purchase_task_id": args.purchase_task_id,
            "supplier_name": args.supplier_name,
        }
        return call_api(
            args.base_url,
            "POST",
            "/api/supplier-aftersale-followups",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-responsibility":
        body = {
            "responsibility_party": args.party,
            "responsibility_note": args.note,
            "supplier_compensation_cents": args.supplier_compensation_cents,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/aftersales/{args.aftersale_id}/responsibility",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-accept":
        body = {
            "address_id": args.address_id,
            "accept_type": args.accept_type,
            "note": args.note,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/aftersales/{args.aftersale_id}/accept",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-reject":
        body = {
            "reject_reason_type": args.reject_reason_type,
            "reject_reason": args.reject_reason,
            "note": args.note,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/aftersales/{args.aftersale_id}/reject",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-reject-reasons":
        query = {
            "shop_id": args.shop_id,
            "reject_scene": args.reject_scene,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/aftersale-reject-reasons",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "aftersale-reject-reasons-sync":
        return call_api(
            args.base_url,
            "POST",
            "/api/aftersale-reject-reasons/sync",
            body={"shop_id": args.shop_id},
            api_key=require_api_key(args.api_key),
        )
    if args.command == "order-profits":
        query = {
            "status": args.status,
            "limit": args.limit,
        }
        return call_api(
            args.base_url,
            "GET",
            "/api/order-profits",
            query=drop_empty(query),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "order-profit-adjustment":
        body = {
            "kind": args.kind,
            "amount_cents": args.amount_cents,
            "note": args.note,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/order-profits/{args.order_id}/adjustments",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "purchase-shipment":
        body = {
            "delivery_id": args.delivery_id,
            "delivery_name": args.delivery_name,
            "waybill_id": args.waybill_id,
            "deliver_type": args.deliver_type,
            "estimated_cost": args.estimated_cost,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/purchase-tasks/{args.purchase_task_id}/shipment",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    if args.command == "purchase-issue":
        body = {
            "issue_type": args.issue_type,
            "note": args.note,
        }
        return call_api(
            args.base_url,
            "POST",
            f"/api/purchase-tasks/{args.purchase_task_id}/issue",
            body=drop_empty(body),
            api_key=require_api_key(args.api_key),
        )
    raise ValueError(f"Unsupported command: {args.command}")


def load_json_payload(args: argparse.Namespace) -> dict[str, Any]:
    if getattr(args, "json", None):
        raw = args.json
    elif getattr(args, "json_file", None) == "-":
        raw = sys.stdin.read()
    elif getattr(args, "json_file", None):
        with open(args.json_file, "r", encoding="utf-8") as file:
            raw = file.read()
    else:
        raise ValueError("Missing JSON payload.")

    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ValueError(f"JSON payload is invalid: {error}") from error
    if not isinstance(payload, dict):
        raise ValueError("JSON payload must be an object.")
    return payload


def call_api(
    base_url: str,
    method: str,
    path: str,
    *,
    query: dict[str, Any] | None = None,
    body: dict[str, Any] | None = None,
    api_key: str | None = None,
    auth: bool = True,
) -> Any:
    url = base_url.rstrip("/") + path
    if query:
        url = f"{url}?{urlencode(query)}"
    data = None
    headers = {"Accept": "application/json"}
    if body is not None:
        data = json.dumps(body, ensure_ascii=False).encode("utf-8")
        headers["Content-Type"] = "application/json"
    if auth:
        headers["x-wx-xd-api-key"] = require_api_key(api_key)
    request = Request(url, data=data, headers=headers, method=method)
    try:
        with urlopen(request, timeout=30) as response:
            return read_json_response(response.read())
    except HTTPError as error:
        raise LocalApiError(error.code, read_json_response(error.read())) from error
    except URLError as error:
        raise LocalApiError(None, {"error": {"message": str(error.reason)}}) from error


def require_api_key(api_key: str | None) -> str:
    if api_key and api_key.strip():
        return api_key.strip()
    raise ValueError(f"Missing API key. Set {API_KEY_ENV} or pass --api-key.")


def read_json_response(raw: bytes) -> Any:
    if not raw:
        return None
    text = raw.decode("utf-8", errors="replace")
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return {"raw": text}


def drop_empty(payload: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in payload.items()
        if value is not None and value != ""
    }


def print_json(payload: Any) -> None:
    print(json.dumps(payload, ensure_ascii=False, indent=2))


def format_error(status: int | None, payload: Any) -> str:
    prefix = f"HTTP {status}" if status is not None else "REQUEST_FAILED"
    return f"{prefix}: {json.dumps(payload, ensure_ascii=False)}"


if __name__ == "__main__":
    raise SystemExit(main())
