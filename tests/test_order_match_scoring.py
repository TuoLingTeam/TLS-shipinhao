# -*- coding: utf-8 -*-

import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from services.order_match_scoring import compute_match_score
from services.review_matcher import BadReviewOrderFinder


def build_evaluation(*, buyer_nickname, product_id, sku_id, product_name, eval_time, evaluation_id="eval-1"):
    return {
        "productEvaluationId": evaluation_id,
        "evaluationInfo": {
            "buyer": {"identity": {"nickname": buyer_nickname}},
            "firstEvaluationInfo": {
                "buyerEvaluationInfo": {
                    "createTime": eval_time,
                    "content": "",
                    "defaultContent": "",
                }
            },
            "evaluationStar": 1,
        },
        "productInfo": {
            "productId": product_id,
            "skuId": sku_id,
            "spuName": product_name,
            "skuName": "默认规格",
        },
        "operationInfo": {},
    }


def build_order(
    *,
    order_id,
    buyer_nickname,
    product_id,
    sku_id,
    product_name,
    create_time,
    confirm_receipt_time,
    status=100,
):
    return {
        "commonInfo": {
            "orderId": order_id,
            "createTime": create_time,
            "status": status,
            "openid": "",
            "isEducationOrder": False,
        },
        "buyerInfo": {"nickName": buyer_nickname},
        "acceptInfo": {"confirmReceiptTime": str(confirm_receipt_time)},
        "orderStatus": {
            "autoConfirmInfo": {
                "isWaybillReceived": False,
                "waybillReceivedTime": 0,
            }
        },
        "orderProductInfo": [
            {
                "productId": product_id,
                "skuId": sku_id,
                "saleParam": "默认规格",
                "title": product_name,
                "thumbImg": "",
            }
        ],
    }


class OrderMatchScoringTests(unittest.TestCase):
    def test_exact_nickname_and_exact_product_should_score_100(self):
        result = compute_match_score(
            evaluation_buyer_nickname="💫其实一个我有故事的人",
            evaluation_product_id="7982968968",
            evaluation_sku_id="7982968968",
            evaluation_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            order_buyer_nickname="💫其实一个我有故事的人",
            order_product_id="7982968968",
            order_sku_id="7982968968",
            order_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
        )

        self.assertEqual(result["score"], 100)
        self.assertTrue(result["buyerNicknameExact"])
        self.assertTrue(result["productExact"])

    def test_raw_nickname_should_not_be_implicitly_cleaned(self):
        result = compute_match_score(
            evaluation_buyer_nickname="💫其实一个我有故事的人",
            evaluation_product_id="7982968968",
            evaluation_sku_id="7982968968",
            evaluation_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            order_buyer_nickname="其实一个我有故事的人",
            order_product_id="7982968968",
            order_sku_id="7982968968",
            order_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
        )

        self.assertFalse(result["buyerNicknameExact"])
        self.assertLess(result["score"], 100)

    def test_matching_should_prefer_exact_raw_nickname_over_wrong_same_product(self):
        """即使正确订单缺少收货参考时间，也必须由昵称+商品信息命中。"""
        finder = BadReviewOrderFinder(cookie="", magic="")
        eval_time = 1_712_910_000
        evaluation = build_evaluation(
            buyer_nickname="无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵",
            product_id="7982968968",
            sku_id="7982968968",
            product_name="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            eval_time=eval_time,
        )
        orders = [
            build_order(
                order_id="3735582233824220672",
                buyer_nickname="Y",
                product_id="7982968968",
                sku_id="7982968968",
                product_name="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
                create_time=eval_time - 172800,
                confirm_receipt_time=eval_time - 86400,
                status=100,
            ),
            build_order(
                order_id="3735563912835389952",
                buyer_nickname="无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵",
                product_id="7982968968",
                sku_id="7982968968",
                product_name="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
                create_time=eval_time - 172800,
                confirm_receipt_time=0,
                status=20,
            ),
        ]

        result = finder.match_orders_with_evaluations([evaluation], orders)[0]
        self.assertEqual(result["orderId"], "3735563912835389952")
        self.assertEqual(result["matchScore"], 100)
        self.assertEqual(result["matchStrategy"], "exact_match")

    def test_score_should_floor_at_50(self):
        result = compute_match_score(
            evaluation_buyer_nickname="甲",
            evaluation_product_id="1",
            evaluation_sku_id="1",
            evaluation_title="AAA",
            order_buyer_nickname="乙",
            order_product_id="2",
            order_sku_id="2",
            order_title="ZZZ",
        )

        self.assertEqual(result["score"], 50)

    def test_changed_nickname_with_numeric_suffix_should_score_high(self):
        result = compute_match_score(
            evaluation_buyer_nickname="赵亮6057",
            evaluation_product_id="7982968968",
            evaluation_sku_id="7982968968",
            evaluation_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            order_buyer_nickname="赵亮",
            order_product_id="7982968968",
            order_sku_id="7982968968",
            order_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
        )

        self.assertEqual(result["score"], 95)

    def test_changed_nickname_with_contained_core_should_score_high(self):
        result = compute_match_score(
            evaluation_buyer_nickname="张皓轩4865",
            evaluation_product_id="7982968968",
            evaluation_sku_id="7982968968",
            evaluation_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            order_buyer_nickname="张皓轩",
            order_product_id="7982968968",
            order_sku_id="7982968968",
            order_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
        )

        self.assertEqual(result["score"], 95)

    def test_changed_nickname_with_subsequence_should_score_high(self):
        result = compute_match_score(
            evaluation_buyer_nickname="潍坊印刷",
            evaluation_product_id="7982968968",
            evaluation_sku_id="7982968968",
            evaluation_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            order_buyer_nickname="潍坊精装印刷王宏杰",
            order_product_id="7982968968",
            order_sku_id="7982968968",
            order_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
        )

        self.assertEqual(result["score"], 90)

    def test_single_character_match_should_not_be_treated_as_high_similarity(self):
        result = compute_match_score(
            evaluation_buyer_nickname="我期待",
            evaluation_product_id="7982968968",
            evaluation_sku_id="7982968968",
            evaluation_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            order_buyer_nickname="我",
            order_product_id="7982968968",
            order_sku_id="7982968968",
            order_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
        )

        self.assertEqual(result["buyerNicknameSimilarity"], 33)
        self.assertEqual(result["score"], 65)

    def test_single_character_containment_in_long_name_should_be_very_low_similarity(self):
        result = compute_match_score(
            evaluation_buyer_nickname="度",
            evaluation_product_id="7982968968",
            evaluation_sku_id="7982968968",
            evaluation_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            order_buyer_nickname="城市轻度假酒店-杨",
            order_product_id="7982968968",
            order_sku_id="7982968968",
            order_title="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
        )

        self.assertLessEqual(result["buyerNicknameSimilarity"], 15)
        self.assertEqual(result["score"], 55)

    def test_single_character_overlap_should_use_ambiguous_reason_text(self):
        finder = BadReviewOrderFinder(cookie="", magic="")
        eval_time = 1_712_910_000
        evaluation = build_evaluation(
            buyer_nickname="度",
            product_id="7982968968",
            sku_id="7982968968",
            product_name="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            eval_time=eval_time,
        )
        orders = [
            build_order(
                order_id="3735245604225966848",
                buyer_nickname="城市轻度假酒店-杨",
                product_id="7982968968",
                sku_id="7982968968",
                product_name="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
                create_time=eval_time - 172800,
                confirm_receipt_time=0,
                status=20,
            ),
        ]

        result = finder.match_orders_with_evaluations([evaluation], orders)[0]
        self.assertIn("昵称仅单字重合，歧义较高！", result["matchReasons"][0])

    def test_multi_character_rename_should_keep_rename_reason_text(self):
        finder = BadReviewOrderFinder(cookie="", magic="")
        eval_time = 1_712_910_000
        evaluation = build_evaluation(
            buyer_nickname="赵亮6057",
            product_id="7982968968",
            sku_id="7982968968",
            product_name="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
            eval_time=eval_time,
        )
        orders = [
            build_order(
                order_id="3735325366963804672",
                buyer_nickname="赵亮",
                product_id="7982968968",
                sku_id="7982968968",
                product_name="仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女",
                create_time=eval_time - 172800,
                confirm_receipt_time=0,
                status=20,
            ),
        ]

        result = finder.match_orders_with_evaluations([evaluation], orders)[0]
        self.assertIn("昵称相似度较高，疑似改名！", result["matchReasons"][0])


if __name__ == "__main__":
    unittest.main()
