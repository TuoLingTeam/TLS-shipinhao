use crate::order_match_scoring::normalize_product_title_for_similarity;
use crate::review_candidate_scoring::{CandidateOrder, EvaluationMatchContext};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndexedOrder {
    pub order: CandidateOrder,
}

pub type ProductIndex = HashMap<String, Vec<IndexedOrder>>;

pub fn build_product_id_key(product_id: &str, sku_id: &str) -> Option<String> {
    if product_id.is_empty() || sku_id.is_empty() {
        None
    } else {
        Some(format!("id::{}::{}", product_id, sku_id))
    }
}

pub fn build_product_value_key(product_name: &str, sku_text: &str) -> Option<String> {
    let name_norm = normalize_product_title_for_similarity(Some(product_name));
    let sku_norm = normalize_product_title_for_similarity(Some(sku_text));
    if name_norm.is_empty() || sku_norm.is_empty() {
        None
    } else {
        Some(format!("value::{}::{}", name_norm, sku_norm))
    }
}

pub fn build_candidate_index_keys(
    evaluation_context: &EvaluationMatchContext,
    sku_name: &str,
) -> Vec<String> {
    let mut keys = Vec::new();
    for key in [
        build_product_id_key(&evaluation_context.product_id, &evaluation_context.sku_id),
        build_product_value_key(&evaluation_context.product_name, sku_name),
    ]
    .into_iter()
    .flatten()
    {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

pub fn build_product_sku_index(orders: &[CandidateOrder]) -> ProductIndex {
    let mut index = ProductIndex::new();
    for order in orders.iter().cloned() {
        for index_key in [
            build_product_id_key(&order.product_id, &order.sku_id),
            build_product_value_key(&order.product_name, &order.sale_param),
        ]
        .into_iter()
        .flatten()
        {
            index.entry(index_key).or_default().push(IndexedOrder {
                order: order.clone(),
            });
        }
    }
    index
}

pub fn collect_candidate_orders(
    product_index: &ProductIndex,
    evaluation_context: &EvaluationMatchContext,
    sku_name: &str,
) -> Vec<CandidateOrder> {
    let mut seen_candidates = HashSet::new();
    let mut candidate_orders = Vec::new();

    for index_key in build_candidate_index_keys(evaluation_context, sku_name) {
        if let Some(items) = product_index.get(&index_key) {
            for item in items {
                let candidate_key = (
                    item.order.order_id.clone(),
                    item.order.product_id.clone(),
                    item.order.sku_id.clone(),
                );
                if seen_candidates.insert(candidate_key) {
                    candidate_orders.push(item.order.clone());
                }
            }
        }
    }

    candidate_orders
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(order_id: &str, sku_text: &str) -> CandidateOrder {
        CandidateOrder {
            order_id: order_id.into(),
            buyer_nickname: "buyer".into(),
            product_id: "p1".into(),
            sku_id: "s1".into(),
            product_name: "仁和洗发水".into(),
            create_time: 1,
            confirm_receipt_time: 0,
            is_waybill_received: false,
            waybill_received_time: 0,
            sale_param: sku_text.into(),
        }
    }

    fn context() -> EvaluationMatchContext {
        EvaluationMatchContext {
            buyer_nickname: "buyer".into(),
            product_id: "p1".into(),
            sku_id: "s1".into(),
            product_name: "仁和洗发水".into(),
            eval_time: 10,
        }
    }

    #[test]
    fn builds_index_keys() {
        assert_eq!(
            build_product_id_key("p1", "s1").as_deref(),
            Some("id::p1::s1")
        );
        assert_eq!(
            build_product_value_key("仁和 洗发水", "默认规格").as_deref(),
            Some("value::仁和洗发水::默认规格")
        );
        assert_eq!(build_product_id_key("", "s1"), None);
    }

    #[test]
    fn builds_index_and_collects_deduplicated_candidates() {
        let a = order("o1", "默认规格");
        let b = order("o2", "默认规格");
        let index = build_product_sku_index(&[a, b]);
        let candidates = collect_candidate_orders(&index, &context(), "默认规格");
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].order_id, "o1");
        assert_eq!(candidates[1].order_id, "o2");
    }

    #[test]
    fn candidate_index_keys_do_not_duplicate() {
        let keys = build_candidate_index_keys(&context(), "默认规格");
        assert_eq!(keys.len(), 2);
    }
}
