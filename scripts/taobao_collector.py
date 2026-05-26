#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import sys
import os
import json
import argparse
import time
import subprocess


def ensure_cloakbrowser():
    """确保 cloakbrowser 可用；缺失时按官方指引执行一次 `pip install cloakbrowser`。"""
    try:
        import cloakbrowser  # noqa: F401
        return
    except ImportError:
        pass
    print("正在安装必要依赖 cloakbrowser...", file=sys.stderr)
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", "cloakbrowser"])
    except subprocess.CalledProcessError as e:
        print(json.dumps({"error": f"安装 cloakbrowser 失败: {e}"}, ensure_ascii=False))
        sys.exit(1)
    try:
        import cloakbrowser  # noqa: F401
    except ImportError as e:
        print(json.dumps({"error": f"安装后仍无法导入 cloakbrowser: {e}"}, ensure_ascii=False))
        sys.exit(1)


def run_login(profile_dir):
    """
    拉起可见浏览器，让用户手动登录淘宝。
    登录态通过 cloakbrowser 持久化 context 自动写入指定的 profile_dir。
    退出条件：context 中所有 page 都已关闭，或与浏览器的连接断开。
    """
    print(f"启动淘宝登录窗口，数据保存在: {profile_dir} ...", file=sys.stderr)
    os.makedirs(profile_dir, exist_ok=True)

    from cloakbrowser import launch_persistent_context

    ctx = None
    try:
        ctx = launch_persistent_context(profile_dir, headless=False, humanize=True)
        pages = ctx.pages if hasattr(ctx, "pages") else []
        page = pages[0] if pages else ctx.new_page()
        page.goto("https://login.taobao.com/", timeout=60000)
        print(
            "请在浏览器中完成淘宝登录（扫码或密码）；登录后关闭浏览器所有窗口即可结束。",
            file=sys.stderr,
        )

        # 轮询：cloakbrowser 的 ctx.pages / page.is_closed() 只是 Python 侧本地缓存，
        # Chromium 进程退出时不会自动更新，必须主动 evaluate 一段 JS 来探测连接。
        # 任何一次探测全部失败即视为浏览器已关闭，结束登录。
        while True:
            time.sleep(1)
            try:
                current_pages = ctx.pages if hasattr(ctx, "pages") else []
            except Exception:
                break
            if not current_pages:
                break
            alive = False
            for p in current_pages:
                try:
                    if p.is_closed():
                        continue
                    p.evaluate("1")
                    alive = True
                    break
                except Exception:
                    continue
            if not alive:
                break

        print("淘宝登录会话已保存。", file=sys.stderr)
    except Exception as e:
        print(f"登录过程出错: {e}", file=sys.stderr)
        sys.exit(1)
    finally:
        # 强制关闭 context，确保 cloakbrowser 拉起的 Chromium 进程退出
        if ctx is not None:
            try:
                ctx.close()
            except Exception:
                pass


def parse_taobao_data(initial_data, source_url):
    """
    解析淘宝详情页的 window.__INITIAL_DATA__ 数据结构
    """
    try:
        # 新版淘宝数据结构解析
        # 提取商品基本信息
        item_info = initial_data.get("item", {})
        title = item_info.get("title", "淘宝未命名商品")
        
        # 提取主图
        images = item_info.get("images", [])
        if not images and "images" in initial_data.get("item", {}):
            images = initial_data["item"]["images"]
        # 确保图片链接为完整 URL
        images = [img if img.startswith("http") else "https:" + img for img in images]
        
        # 提取详情图
        desc_info = initial_data.get("desc", {})
        detail_images = []
        if "descDetailInfo" in desc_info:
            detail_imgs_list = desc_info.get("descDetailInfo", {}).get("images", [])
            detail_images = [img if img.startswith("http") else "https:" + img for img in detail_imgs_list]
        
        # 提取 SKU 规格和价格信息
        skus = []
        sku_info = initial_data.get("sku", {})
        val_item_info = sku_info.get("valItemInfo", {})
        sku_list = val_item_info.get("skuList", [])
        
        # 规格属性映射表，如 {"12345": "红色", "67890": "L码"}
        # 新版淘宝通常把 specs 保存在 sku.valItemInfo 中
        prop_path_map = {}
        for prop in val_item_info.get("props", []):
            prop_name = prop.get("name", "")
            for val in prop.get("values", []):
                val_id = val.get("valueId", "")
                val_name = val.get("name", "")
                if prop_name and val_id and val_name:
                    prop_path_map[str(val_id)] = (prop_name, val_name)
                    
        # 价格与库存映射
        # 在 skuCore 或是 priceDepot 结构中
        sku_core = initial_data.get("skuCore", {})
        sku_2_info = sku_core.get("sku2info", {})
        
        # 遍历 SKU 列表，组装外部 SKU 输入格式
        for idx, sku_item in enumerate(sku_list):
            sku_id = str(sku_item.get("skuId", f"sku_{idx}"))
            p_path = sku_item.get("pPath", "")  # 格式如 "20509:28314;1627207:28320"
            
            # 解析规格 specs
            specs = {}
            if p_path:
                # 切割规格对
                parts = p_path.split(";")
                for part in parts:
                    if ":" in part:
                        _, val_id = part.split(":", 1)
                        if val_id in prop_path_map:
                            p_name, v_name = prop_path_map[val_id]
                            specs[p_name] = v_name
            
            # 获取价格与库存
            # 淘宝价格通常是分，需转为元
            sku_details = sku_2_info.get(sku_id, {})
            price_cents = 0
            if "price" in sku_details:
                price_cents = int(sku_details.get("price", {}).get("priceMoney", 0))
            elif "promotion" in sku_details:
                price_cents = int(sku_details.get("promotion", {}).get("priceMoney", 0))
                
            cost_price = float(price_cents) / 100.0 if price_cents > 0 else 9.9
            
            # 如果没拿到规格或者规格为空，可以回退为默认规格
            if not specs:
                specs = {"规格": sku_item.get("names", "默认规格")}
                
            stock = int(sku_details.get("quantity", 99))
            
            skus.append({
                "external_sku_id": sku_id,
                "specs": specs,
                "cost_price": cost_price,
                "stock": stock
            })
            
        # 如果解析失败或者没有 SKU，兜底生成一个默认 SKU
        if not skus:
            skus.append({
                "external_sku_id": "default",
                "specs": {"规格": "默认规格"},
                "cost_price": 9.9,
                "stock": 99
            })
            
        return {
            "external_product_id": source_url,
            "title": title,
            "source_url": source_url,
            "images": images,
            "detail_images": detail_images,
            "skus": skus,
            "supplier_name": "淘宝商家",
            "supplier_product_id": source_url,
            "category_hint": None,
            "brand_hint": "无品牌",
            "weight_gram": 500,
            "metadata": {}
        }
    except Exception as e:
        raise ValueError(f"淘宝数据结构解析失败: {str(e)}")


def run_check_login(profile_dir):
    """
    检测当前 profile_dir 是否含有效淘宝登录态。
    策略：直接读取 persistent context 里的 cookies，检查是否存在
    淘宝登录身份相关的 cookie（unb / tracknick / cookie2 / sgcookie）。
    这种方式不依赖页面渲染，不会触发反爬，与 headed Chromium 启动时
    呈现的登录态来源一致。
    输出 stdout：{"logged_in": bool, "detail": str, "matched_cookies": [...]}
    """
    os.makedirs(profile_dir, exist_ok=True)

    from cloakbrowser import launch_persistent_context

    # 任一存在即视为已登录（unb 最强；其它为辅助判定）
    LOGIN_COOKIE_NAMES = {"unb", "tracknick", "cookie2", "sgcookie", "_l_g_"}

    ctx = None
    try:
        ctx = launch_persistent_context(profile_dir, headless=True, humanize=True)

        cookies = []
        try:
            cookies = ctx.cookies()
        except Exception:
            # 部分实现需要传 url 才能取到 cookies
            try:
                cookies = ctx.cookies(["https://www.taobao.com", "https://login.taobao.com"])
            except Exception:
                cookies = []

        matched = []
        for c in cookies or []:
            name = c.get("name") if isinstance(c, dict) else getattr(c, "name", None)
            domain = c.get("domain", "") if isinstance(c, dict) else getattr(c, "domain", "") or ""
            value = c.get("value", "") if isinstance(c, dict) else getattr(c, "value", "") or ""
            if not name or not value:
                continue
            if "taobao.com" not in domain and "tmall.com" not in domain:
                continue
            if name in LOGIN_COOKIE_NAMES:
                matched.append(name)

        logged_in = len(matched) > 0
        if logged_in:
            detail = f"已检测到有效的淘宝登录态（匹配 cookie: {', '.join(sorted(set(matched)))}）。"
        else:
            detail = "未在 profile 中检测到淘宝登录 cookie，请先完成登录。"

        print(json.dumps(
            {"logged_in": logged_in, "detail": detail, "matched_cookies": sorted(set(matched))},
            ensure_ascii=False,
        ))
        sys.exit(0)
    except Exception as e:
        print(json.dumps({"error": f"登录态检测失败: {e}"}, ensure_ascii=False))
        sys.exit(1)
    finally:
        if ctx is not None:
            try:
                ctx.close()
            except Exception:
                pass


def run_collect(url, profile_dir, headed=False):
    """
    加载淘宝登录态 profile，使用 cloakbrowser 持久化 context 爬取淘宝商品页，
    提取 window.__INITIAL_DATA__，解析后将符合 ExternalProductInput 格式的 JSON 打印到 stdout。
    """
    os.makedirs(profile_dir, exist_ok=True)

    from cloakbrowser import launch_persistent_context

    ctx = None
    try:
        # 持久化 context 复用淘宝登录态；headed=True 用于反爬严重时手工调试
        ctx = launch_persistent_context(profile_dir, headless=not headed, humanize=True)
        pages = ctx.pages if hasattr(ctx, "pages") else []
        page = pages[0] if pages else ctx.new_page()

        # 导航到淘宝详情页
        page.goto(url, timeout=30000)

        # 等待页面中的关键元素或者等待一段时间
        # 确保淘宝页面脚本执行完成
        page.wait_for_timeout(3000)

        # 获取 window.__INITIAL_DATA__
        initial_data = page.evaluate("() => window.__INITIAL_DATA__")

        # 如果没有获取到 INITIAL_DATA，尝试等待更长，或者滑动页面触发脚本
        if not initial_data:
            # 模拟页面微小滚动，触发淘宝防懒加载和加载核心变量
            page.evaluate("window.scrollBy(0, 300)")
            page.wait_for_timeout(2000)
            initial_data = page.evaluate("() => window.__INITIAL_DATA__")

        if not initial_data:
            # 天猫页面可能会把数据挂在 window.g_config 下，或者通过抓取页面 HTML 匹配
            html_content = page.content()
            # 简单判断是否进入了淘宝滑块验证码页面
            if "验证码" in html_content or "punish" in page.url:
                raise RuntimeError("被淘宝滑块验证码拦截，请重新进行淘宝登录或尝试手动滑块。")
            raise RuntimeError("无法在页面上获取到 window.__INITIAL_DATA__，可能需要重新登录。")

        # 解析数据
        parsed_product = parse_taobao_data(initial_data, url)

        # 如果详情图为空，可以在这里多等待或尝试拉取详情接口数据
        # 淘宝详情图如果是懒加载，我们可以通过向下滚动来加载
        if not parsed_product["detail_images"]:
            page.evaluate("window.scrollBy(0, 1500)")
            page.wait_for_timeout(2000)
            # 重新获取 desc images
            initial_data = page.evaluate("() => window.__INITIAL_DATA__")
            if initial_data:
                desc_info = initial_data.get("desc", {})
                if "descDetailInfo" in desc_info:
                    detail_imgs_list = desc_info.get("descDetailInfo", {}).get("images", [])
                    parsed_product["detail_images"] = [img if img.startswith("http") else "https:" + img for img in detail_imgs_list]

        # 将解析后的商品数据打印至标准输出 (stdout)
        print(json.dumps(parsed_product, ensure_ascii=False))
        sys.exit(0)

    except Exception as e:
        # 出错输出错误 JSON
        print(json.dumps({"error": str(e)}, ensure_ascii=False))
        sys.exit(1)
    finally:
        if ctx is not None:
            try:
                ctx.close()
            except Exception:
                pass


def main():
    parser = argparse.ArgumentParser(description="淘宝商品详情反检测采集脚本（CloakBrowser 驱动）")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # 登录子命令
    login_parser = subparsers.add_parser("login", help="手动登录淘宝以获取并保存 Cookie/Profile")
    login_parser.add_argument("--profile-dir", required=True, help="Profile 保存的绝对路径")

    # 登录态检测子命令
    check_parser = subparsers.add_parser("check-login", help="检测当前 Profile 是否含有效淘宝登录态")
    check_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")

    # 采集子命令
    collect_parser = subparsers.add_parser("collect", help="自动采集淘宝商品详细数据")
    collect_parser.add_argument("--url", required=True, help="淘宝商品链接")
    collect_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")
    collect_parser.add_argument("--headed", action="store_true", help="以可见窗口方式运行（反爬严重时使用）")

    args = parser.parse_args()

    ensure_cloakbrowser()

    if args.command == "login":
        run_login(args.profile_dir)
    elif args.command == "check-login":
        run_check_login(args.profile_dir)
    elif args.command == "collect":
        run_collect(args.url, args.profile_dir, headed=bool(getattr(args, "headed", False)))


if __name__ == "__main__":
    main()
