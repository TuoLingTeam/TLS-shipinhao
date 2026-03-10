#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
完整版差评订单查找器 - 使用5属性精确匹配
匹配属性: productId, skuId, saleParam, nickname, product_id
"""

import requests
import json
import os
import time
from datetime import datetime, timedelta

class BadReviewOrderFinder:
    def __init__(self):
        self.evaluation_url = "https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search"
        self.order_url = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch"
        
        # 加载认证信息
        self.cookie, self.magic_values = self.load_cookie()
        if not self.cookie:
            raise Exception("Cookie加载失败")
    
    def load_cookie(self):
        """加载cookie并提取magic值"""
        if not os.path.exists("cookie.txt"):
            print("❌ Cookie文件不存在")
            return None, {}
        
        with open("cookie.txt", 'r', encoding='utf-8') as f:
            content = f.read().strip()
        
        if content and not content.startswith('#'):
            print("✅ Cookie加载成功")
            
            # 提取magic值
            magic_values = {}
            magic_keys = ['biz_magic']
            
            for key in magic_keys:
                if f'{key}=' in content:
                    for part in content.split(';'):
                        part = part.strip()
                        if part.startswith(f'{key}='):
                            value = part.split('=', 1)[1]
                            magic_values[key] = value
                            print(f"✅ 提取到{key}")
                            break
            
            return content, magic_values
        else:
            print("❌ Cookie文件为空或只有注释")
            return None, {}
    
    def get_headers(self, referer='https://store.weixin.qq.com/shop/evaluate/home'):
        """获取标准请求头"""
        headers = {
            'Accept': 'application/json, text/plain, */*',
            'Accept-Encoding': 'gzip, deflate, br, zstd',
            'Accept-Language': 'zh-CN,zh;q=0.9',
            'Connection': 'keep-alive',
            'Content-Type': 'application/json',
            'Host': 'store.weixin.qq.com',
            'Origin': 'https://store.weixin.qq.com',
            'Referer': referer,
            'Sec-Fetch-Dest': 'empty',
            'Sec-Fetch-Mode': 'cors',
            'Sec-Fetch-Site': 'same-origin',
            'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36',
            'sec-ch-ua': '"Not(A:Brand";v="8", "Chromium";v="144", "Google Chrome";v="144"',
            'sec-ch-ua-mobile': '?0',
            'sec-ch-ua-platform': '"macOS"',
            'Cookie': self.cookie,
            'biz_magic': self.magic_values.get('biz_magic', ''),
            'potter-scene': 'weixinShop'
        }
        return headers
    
    def get_bad_evaluations(self, days=30):
        """获取差评数据"""
        print("获取差评数据...")
        
        end_time = int(time.time())
        start_time = int((datetime.now() - timedelta(days=days)).timestamp())
        
        all_bad_reviews = []
        page = 1
        max_pages = 10
        
        while page <= max_pages:
            print(f"正在获取第 {page} 页评价...", end='', flush=True)
            
            params = {'token': '', 'lang': 'zh_CN'}
            data = {
                'orderId': '',
                'productId': '',
                'productEvaluationId': '',
                'buyerEvaluationTimeStart': start_time,
                'buyerEvaluationTimeEnd': end_time,
                'page': page,
                'status': 2,
                'visibleType': 0
            }
            
            try:
                response = requests.post(
                    self.evaluation_url,
                    params=params,
                    json=data,
                    headers=self.get_headers(),
                    timeout=30
                )
                
                if response.status_code in [200, 201]:
                    result = response.json()
                    if result.get('code') == 0:
                        evaluations = result.get('finderProductEvaluationInfoList', [])
                        
                        page_bad_reviews = []
                        for evaluation in evaluations:
                            operation_info = evaluation.get('operationInfo', {})
                            attitude_name = operation_info.get('attitudeName', '')
                            can_reply_expire_time = operation_info.get('canReplyExpireTime', 0)
                            
                            if attitude_name == '不够好':
                                expire_date = datetime.fromtimestamp(can_reply_expire_time)
                                now = datetime.now()
                                days_until_expire = (expire_date - now).days
                                
                                if days_until_expire >= -30:
                                    page_bad_reviews.append(evaluation)
                        
                        all_bad_reviews.extend(page_bad_reviews)
                        print(f" 获取到 {len(page_bad_reviews)} 条差评 (总计: {len(all_bad_reviews)})")
                        
                        if len(evaluations) < 10:
                            print("已获取全部差评数据")
                            break
                        
                        page += 1
                        time.sleep(1)
                    else:
                        print(f"差评API错误: {result}")
                        break
                else:
                    print(f"差评请求失败: {response.status_code}")
                    break
                    
            except Exception as e:
                print(f"差评请求异常: {e}")
                break
        
        print(f"差评获取完成，共 {len(all_bad_reviews)} 条")
        return all_bad_reviews
    
    def get_orders(self, max_pages=None):
        """获取订单数据 - 获取全部订单，优化版本"""
        print("获取订单数据...")
        
        all_orders = []
        next_key = ""
        page = 1
        
        headers = self.get_headers('https://store.weixin.qq.com/shop/order/list')
        
        while True:
            print(f"正在获取第 {page} 页订单...", end='', flush=True)
            
            params = {'token': '', 'lang': 'zh_CN'}
            data = {
                'pageSize': 100,  # API限制最大100
                'nextKey': next_key,
                'orderStatus': '',
                'searchType': 0
            }
            
            try:
                response = requests.post(
                    self.order_url,
                    params=params,
                    json=data,
                    headers=headers,
                    timeout=30
                )
                
                if response.status_code in [200, 201]:
                    result = response.json()
                    if result.get('code') == 0:
                        orders = result.get('orderList', [])
                        next_key = result.get('nextKey', '')
                        
                        all_orders.extend(orders)
                        print(f" 获取到 {len(orders)} 个订单 (总计: {len(all_orders)})")
                        
                        # 如果没有更多数据或订单为空，停止获取
                        if not next_key or not orders:
                            print("已获取全部订单数据")
                            break
                        
                        # 如果设置了最大页数限制
                        if max_pages and page >= max_pages:
                            print(f"达到最大页数限制 {max_pages}")
                            break
                        
                        page += 1
                        # 移除延迟以最大化速度，如果出现问题再调整
                        # time.sleep(0.2)
                    else:
                        print(f"订单API错误: {result}")
                        break
                else:
                    print(f"订单请求失败: {response.status_code}")
                    break
                    
            except Exception as e:
                print(f"订单请求异常: {e}")
                break
        
        print(f"订单获取完成，共 {len(all_orders)} 个订单")
        return all_orders
    
    def normalize_nickname(self, nickname):
        """标准化昵称，移除emoji和特殊字符"""
        if not nickname:
            return ""
        
        # 移除常见emoji符号（使用简单的字符替换，避免误删中文）
        emoji_chars = ['🌈', '⭐', '💎', '🔥', '✨', '🎉', '🎊', '💫', '🌟', '❤️', '💕', '💖', '💗', '💘', '💙', '💚', '💛', '💜', '🧡', '🖤', '🤍', '🤎', '💯', '💢', '💥', '💫', '💦', '💨', '🕳️', '💣', '💬', '👁️‍🗨️', '🗨️', '🗯️', '💭', '💤']
        
        result = nickname
        for emoji in emoji_chars:
            result = result.replace(emoji, '')
        
        # 去除首尾空格
        return result.strip()
    
    def match_orders_with_evaluations(self, bad_evaluations, orders):
        """使用增强匹配策略：商品匹配 + 时间窗口匹配 + 买家特征匹配"""
        print("\n开始增强匹配订单和评价...")
        print("匹配策略: 商品匹配 + 时间窗口匹配 + 买家特征匹配")
        
        # 构建多层索引
        print("构建增强索引...")
        
        # 按商品ID+SKU ID分组的索引
        product_sku_index = {}
        
        for order in orders:
            order_id = order.get('commonInfo', {}).get('orderId')
            buyer_nickname = order.get('buyerInfo', {}).get('nickName', '')
            normalized_buyer_nickname = self.normalize_nickname(buyer_nickname)
            create_time = order.get('commonInfo', {}).get('createTime', 0)
            
            # 获取收货确认时间
            confirm_receipt_time = order.get('acceptInfo', {}).get('confirmReceiptTime', '')
            confirm_receipt_timestamp = 0
            if confirm_receipt_time and confirm_receipt_time.isdigit():
                confirm_receipt_timestamp = int(confirm_receipt_time)
            
            # 获取订单状态
            order_status = order.get('commonInfo', {}).get('status', 0)
            
            # 获取买家信息
            openid = order.get('commonInfo', {}).get('openid', '')
            
            order_products = order.get('orderProductInfo', [])
            for product in order_products:
                product_id = product.get('productId')
                sku_id = product.get('skuId')
                sale_params = product.get('saleParam', [])
                sale_param_str = '|'.join(sale_params) if sale_params else ''
                
                if product_id and sku_id:
                    order_data = {
                        'orderId': order_id,
                        'productId': product_id,
                        'skuId': sku_id,
                        'saleParam': sale_param_str,
                        'buyerNickname': buyer_nickname,
                        'normalizedNickname': normalized_buyer_nickname,
                        'createTime': create_time,
                        'confirmReceiptTime': confirm_receipt_timestamp,
                        'orderStatus': order_status,
                        'openid': openid,
                        'orderData': order
                    }
                    
                    # 构建商品+SKU索引
                    product_sku_key = f"{product_id}_{sku_id}"
                    if product_sku_key not in product_sku_index:
                        product_sku_index[product_sku_key] = []
                    product_sku_index[product_sku_key].append(order_data)
        
        print(f"构建了商品+SKU索引: {len(product_sku_index)} 个键")
        
        # 匹配评价
        results = []
        matched_count = 0
        strategy_stats = {'exact_match': 0, 'time_window': 0, 'buyer_feature': 0, 'fallback': 0}
        
        for i, evaluation in enumerate(bad_evaluations, 1):
            eval_info = evaluation.get('evaluationInfo', {})
            product_info = evaluation.get('productInfo', {})
            operation_info = evaluation.get('operationInfo', {})
            buyer_identity = evaluation.get('buyerIdentity', {})
            
            evaluation_id = evaluation.get('productEvaluationId')
            product_id = product_info.get('productId')
            sku_id = product_info.get('skuId')
            sku_name = product_info.get('skuName', '')
            
            buyer_nickname = eval_info.get('buyer', {}).get('identity', {}).get('nickname', '')
            normalized_buyer_nickname = self.normalize_nickname(buyer_nickname)
            eval_time = eval_info.get('firstEvaluationInfo', {}).get('buyerEvaluationInfo', {}).get('createTime', 0)
            
            # 获取买家头像信息
            buyer_headimg1 = buyer_identity.get('headimgurl', '')
            buyer_headimg2 = eval_info.get('buyerHeadimgurl', '')
            buyer_type = buyer_identity.get('type', 0)
            
            print(f"[{i}/{len(bad_evaluations)}] 处理评价 {evaluation_id}: {buyer_nickname}")
            
            matched_order = None
            match_strategy = None
            match_score = 0
            
            if product_id and sku_id:
                product_sku_key = f"{product_id}_{sku_id}"
                candidate_orders = product_sku_index.get(product_sku_key, [])
                
                if candidate_orders:
                    print(f"  找到 {len(candidate_orders)} 个候选订单")
                    
                    best_matches = []
                    
                    for order_data in candidate_orders:
                        score = 0
                        reasons = []
                        
                        # 1. 有效评价时间窗口 (最高优先级 - 35分)
                        # 根据官方规则：只有收货后30天内的首次评价才有效且计入店铺体验分
                        reference_time = order_data['confirmReceiptTime'] if order_data['confirmReceiptTime'] > 0 else order_data['createTime']
                        
                        if eval_time > 0 and reference_time > 0:
                            if eval_time > reference_time:
                                time_diff_hours = (eval_time - reference_time) / 3600
                                time_diff_days = time_diff_hours / 24
                                
                                if time_diff_days <= 30:  # 30天内 - 有效首次评价期
                                    if time_diff_days <= 1:  # 1天内 - 最有效
                                        score += 35
                                        reasons.append(f"有效评价期-极及时({time_diff_days:.1f}天)")
                                    elif time_diff_days <= 7:  # 7天内 - 很有效
                                        score += 30
                                        reasons.append(f"有效评价期-很及时({time_diff_days:.1f}天)")
                                    elif time_diff_days <= 15:  # 15天内 - 有效
                                        score += 25
                                        reasons.append(f"有效评价期-及时({time_diff_days:.1f}天)")
                                    else:  # 15-30天 - 勉强有效
                                        score += 20
                                        reasons.append(f"有效评价期-正常({time_diff_days:.1f}天)")
                                else:  # 超过30天 - 无效评价（系统自动评价或追评）
                                    score += 0  # 0分，直接跳过
                                    continue
                            else:
                                # 评价时间早于参考时间，不合理
                                continue
                        else:
                            # 没有时间信息，无法判断有效性
                            continue
                        
                        # 2. 买家身份匹配 (第二优先级 - 30分)
                        if normalized_buyer_nickname and order_data['normalizedNickname']:
                            if normalized_buyer_nickname == order_data['normalizedNickname']:
                                score += 30
                                reasons.append("买家昵称完全匹配")
                            elif len(normalized_buyer_nickname) >= 2 and len(order_data['normalizedNickname']) >= 2:
                                # 昵称部分匹配（至少2个字符）
                                if normalized_buyer_nickname in order_data['normalizedNickname'] or order_data['normalizedNickname'] in normalized_buyer_nickname:
                                    score += 15
                                    reasons.append("买家昵称部分匹配")
                        
                        # 3. 基础商品匹配 (第三优先级 - 20分，必须条件)
                        if sku_name in order_data['saleParam']:
                            score += 20
                            reasons.append("商品规格完全匹配")
                        else:
                            continue  # 商品规格不匹配，直接跳过
                        
                        # 4. 订单完成状态 (第四优先级 - 10分)
                        # 只有已完成的订单才能正常评价
                        if order_data['orderStatus'] >= 100:  # 已完成
                            score += 10
                            reasons.append("订单已完成")
                        elif order_data['orderStatus'] >= 60:  # 已发货
                            score += 5
                            reasons.append("订单已发货")
                        else:
                            score += 0  # 未完成订单不太可能有评价
                            reasons.append("订单未完成")
                        
                        # 5. 评价及时性 (第五优先级 - 5分)
                        # 基于收货确认时间的评价响应速度
                        if eval_time > 0 and order_data['confirmReceiptTime'] > 0:
                            confirm_diff = abs(eval_time - order_data['confirmReceiptTime'])
                            confirm_diff_hours = confirm_diff / 3600
                            confirm_diff_days = confirm_diff_hours / 24
                            
                            if confirm_diff < 3600:  # 1小时内 - 立即评价
                                score += 5
                                reasons.append(f"收货后立即评价({confirm_diff}秒)")
                            elif confirm_diff_days <= 1:  # 1天内 - 很快
                                score += 4
                                reasons.append(f"收货后当天评价({confirm_diff_hours:.1f}小时)")
                            elif confirm_diff_days <= 7:  # 7天内 - 较快
                                score += 3
                                reasons.append(f"收货后一周内评价({confirm_diff_days:.1f}天)")
                            else:  # 7-30天内 - 正常
                                score += 2
                                reasons.append(f"收货后月内评价({confirm_diff_days:.1f}天)")
                        
                        # 只保留得分较高的候选 (最低阈值调整为40分)
                        # 有效评价期(20) + 商品匹配(20) = 40分最低门槛
                        if score >= 40:
                            best_matches.append({
                                'order_data': order_data,
                                'score': score,
                                'reasons': reasons,
                                'time_diff': abs(eval_time - order_data['createTime']) if eval_time > 0 and order_data['createTime'] > 0 else float('inf'),
                                'confirm_diff': abs(eval_time - order_data['confirmReceiptTime']) if eval_time > 0 and order_data['confirmReceiptTime'] > 0 else float('inf')
                            })
                    
                    # 选择最佳匹配
                    if best_matches:
                        # 按得分排序，得分相同时按时间差排序
                        best_matches.sort(key=lambda x: (-x['score'], x['confirm_diff'], x['time_diff']))
                        best_match = best_matches[0]
                        
                        matched_order = best_match['order_data']
                        match_score = best_match['score']
                        
                        # 确定匹配策略
                        if match_score >= 80:
                            match_strategy = 'exact_match'
                        elif match_score >= 50:
                            match_strategy = 'time_window'
                        elif match_score >= 30:
                            match_strategy = 'buyer_feature'
                        else:
                            match_strategy = 'fallback'
                        
                        strategy_stats[match_strategy] += 1
                        matched_count += 1
                        
                        print(f"  ✅ {match_strategy}匹配成功 (得分: {match_score})")
                        print(f"  ✅ 匹配原因: {', '.join(best_match['reasons'])}")
                        print(f"  ✅ 最终匹配订单: {matched_order['orderId']}")
                        
                        # 显示前3个候选的得分情况
                        if len(best_matches) > 1:
                            print(f"  📊 候选订单得分: ", end="")
                            for j, candidate in enumerate(best_matches[:3]):
                                print(f"#{j+1}({candidate['score']}分)", end=" ")
                            print()
            
            if not matched_order:
                print(f"  ❌ 未找到匹配订单")
            
            # 构建结果
            result = {
                'evaluationId': evaluation_id,
                'orderId': matched_order['orderId'] if matched_order else None,
                'productId': product_id,
                'skuId': sku_id,
                'skuName': sku_name,
                'saleParam': matched_order['saleParam'] if matched_order else '',
                'buyerNickname': buyer_nickname,
                'orderBuyerNickname': matched_order['buyerNickname'] if matched_order else '',
                'matchStrategy': match_strategy if matched_order else None,
                'matchScore': match_score if matched_order else 0,
                'timeDiffHours': (eval_time - matched_order['createTime']) / 3600 if matched_order and eval_time > 0 and matched_order['createTime'] > 0 else None,
                'confirmDiffHours': (eval_time - matched_order['confirmReceiptTime']) / 3600 if matched_order and eval_time > 0 and matched_order['confirmReceiptTime'] > 0 else None,
                'attitudeName': operation_info.get('attitudeName', ''),
                'evaluationContent': eval_info.get('firstEvaluationInfo', {}).get('buyerEvaluationInfo', {}).get('content', ''),
                'defaultContent': eval_info.get('firstEvaluationInfo', {}).get('buyerEvaluationInfo', {}).get('defaultContent', ''),
                'evaluationStar': eval_info.get('evaluationStar', 0),
                'productName': product_info.get('spuName', ''),
                'canReplyExpireTime': operation_info.get('canReplyExpireTime', 0),
                'matched': matched_order is not None
            }
            
            results.append(result)
        
        print(f"\n增强匹配完成：{matched_count}/{len(bad_evaluations)} 条评价找到了对应订单")
        print(f"匹配成功率: {matched_count/len(bad_evaluations)*100:.1f}%")
        print(f"匹配策略统计: 精确匹配={strategy_stats['exact_match']}, 时间窗口={strategy_stats['time_window']}, 买家特征={strategy_stats['buyer_feature']}, 降级匹配={strategy_stats['fallback']}")
        
        return results
    
    def run(self):
        """主执行流程"""
        print("=== 差评订单查找器启动 ===\n")
        
        # 1. 获取差评数据
        bad_evaluations = self.get_bad_evaluations(days=30)
        if not bad_evaluations:
            print("❌ 没有找到差评")
            return []
        
        # 2. 获取订单数据 - 获取全部订单
        orders = self.get_orders()  # 不限制页数，获取全部
        if not orders:
            print("❌ 没有获取到订单数据")
            return []
        
        # 3. 精确匹配
        results = self.match_orders_with_evaluations(bad_evaluations, orders)
        
        # 4. 保存结果
        filename = f"中差评数据_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        with open(filename, 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print(f"\n✅ 结果已保存到: {filename}")
        
        # 5. 显示匹配的订单ID
        matched_results = [r for r in results if r['matched']]
        if matched_results:
            print(f"\n=== 匹配成功的订单 ===")
            for result in matched_results:
                expire_time = datetime.fromtimestamp(result['canReplyExpireTime']).strftime('%Y-%m-%d %H:%M')
                print(f"订单ID: {result['orderId']}")
                print(f"  买家: {result['buyerNickname']}")
                print(f"  商品: {result['productName']}")
                print(f"  回复期限: {expire_time}")
                print()
        
        return results


def main():
    try:
        finder = BadReviewOrderFinder()
        results = finder.run()
        
        matched_count = sum(1 for r in results if r['matched'])
        print(f"✅ 执行完成，{matched_count}/{len(results)} 条差评匹配到订单")
        
    except Exception as e:
        print(f"❌ 执行失败: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    main()
