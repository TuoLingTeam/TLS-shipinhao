use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DELIVERY_MISMATCH_MESSAGE: &str = "快递单号与所选物流商不匹配";
const DELIVERY_MISMATCH_MARKERS: [&str; 2] = [DELIVERY_MISMATCH_MESSAGE, "快递单号有误"];
const DELIVERY_NON_RETRYABLE_MARKERS: [&str; 2] = ["订单已确认收货", "不支持修改物流"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryProductItem {
    pub product_id: String,
    pub sku_id: String,
    pub product_cnt: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryProductInfo {
    pub delivery_id: String,
    pub delivery_name: String,
    pub waybill_id: String,
    pub deliver_type: Option<i64>,
    pub waybill_status: Option<i64>,
    pub is_all_product: Option<bool>,
    pub delivery_time: Option<String>,
    #[serde(default)]
    pub product_infos: Vec<DeliveryProductItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliverySnapshot {
    pub delivery_id: String,
    pub delivery_name: String,
    pub waybill_id: String,
    #[serde(default)]
    pub product_infos: Vec<DeliveryProductItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryOverride {
    pub delivery_id: String,
    pub delivery_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryCandidate {
    pub delivery_id: String,
    pub delivery_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryChange {
    pub old: DeliveryProductInfo,
    pub new: DeliveryProductInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryUpdatePayload {
    pub order_id: String,
    #[serde(default)]
    pub change_info: Vec<DeliveryChange>,
}

pub fn normalize_product_infos(
    delivery_product_info: &DeliveryProductInfo,
) -> Vec<DeliveryProductItem> {
    delivery_product_info
        .product_infos
        .iter()
        .filter(|item| !item.product_id.is_empty() && !item.sku_id.is_empty())
        .cloned()
        .collect()
}

pub fn extract_delivery_snapshot(
    delivery_product_info: &DeliveryProductInfo,
) -> anyhow::Result<DeliverySnapshot> {
    if delivery_product_info.delivery_id.is_empty() {
        anyhow::bail!("获取订单详情失败：订单详情缺少承运商信息（deliveryId）。")
    }
    let product_infos = normalize_product_infos(delivery_product_info);
    if product_infos.is_empty() {
        anyhow::bail!("获取订单详情失败：订单详情缺少商品信息，无法更新物流。")
    }
    Ok(DeliverySnapshot {
        delivery_id: delivery_product_info.delivery_id.clone(),
        delivery_name: delivery_product_info.delivery_name.clone(),
        waybill_id: delivery_product_info.waybill_id.clone(),
        product_infos,
    })
}

pub fn build_delivery_candidates(
    tracking_number: &str,
    delivery_snapshot: &DeliverySnapshot,
) -> Vec<DeliveryCandidate> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut push = |delivery_id: &str, delivery_name: &str| {
        if delivery_id.is_empty() {
            return;
        }
        let key = (delivery_id.to_string(), delivery_name.to_string());
        if seen.insert(key.clone()) {
            candidates.push(DeliveryCandidate {
                delivery_id: key.0,
                delivery_name: key.1,
            });
        }
    };
    push(
        &delivery_snapshot.delivery_id,
        &delivery_snapshot.delivery_name,
    );
    let prefix = tracking_number.trim().chars().take(2).collect::<String>();
    push(&prefix, &delivery_snapshot.delivery_name);
    candidates
}

pub fn build_update_delivery_payload(
    order_id: &str,
    tracking_number: &str,
    old_delivery_product_info: &DeliveryProductInfo,
    delivery_override: Option<&DeliveryOverride>,
) -> DeliveryUpdatePayload {
    let old_info = old_delivery_product_info.clone();
    let mut new_info = old_delivery_product_info.clone();
    new_info.waybill_id = tracking_number.trim().to_string();
    if let Some(override_info) = delivery_override {
        if !override_info.delivery_id.is_empty() {
            new_info.delivery_id = override_info.delivery_id.clone();
        }
        if !override_info.delivery_name.is_empty() {
            new_info.delivery_name = override_info.delivery_name.clone();
        }
    }
    DeliveryUpdatePayload {
        order_id: order_id.trim().to_string(),
        change_info: vec![DeliveryChange {
            old: old_info,
            new: new_info,
        }],
    }
}

pub fn build_raw_update_delivery_payload(
    order_id: &str,
    tracking_number: &str,
    old_delivery_product_info: &Value,
    delivery_override: Option<&DeliveryOverride>,
) -> anyhow::Result<Value> {
    let mut new_info = old_delivery_product_info.clone();
    let obj = new_info
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("原始物流快照格式无效"))?;
    obj.insert(
        "waybillId".to_string(),
        Value::String(tracking_number.trim().to_string()),
    );
    if let Some(override_info) = delivery_override {
        if !override_info.delivery_id.is_empty() {
            obj.insert(
                "deliveryId".to_string(),
                Value::String(override_info.delivery_id.clone()),
            );
        }
        if !override_info.delivery_name.is_empty() {
            obj.insert(
                "deliveryName".to_string(),
                Value::String(override_info.delivery_name.clone()),
            );
        }
    }
    Ok(serde_json::json!({
        "orderId": order_id.trim(),
        "changeInfo": [{
            "old": old_delivery_product_info,
            "new": new_info,
        }],
    }))
}

pub fn is_delivery_mismatch_error(message: &str) -> bool {
    DELIVERY_MISMATCH_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
}

pub fn is_non_retryable_delivery_error(message: &str) -> bool {
    DELIVERY_NON_RETRYABLE_MARKERS
        .iter()
        .all(|marker| message.contains(marker))
}

pub fn determine_delivery_override_on_mismatch(
    tracking_number: &str,
    delivery_product_info: &DeliveryProductInfo,
) -> Option<DeliveryOverride> {
    let prefix = tracking_number.trim().chars().take(2).collect::<String>();
    if prefix.is_empty() || prefix == delivery_product_info.delivery_id {
        return None;
    }
    Some(DeliveryOverride {
        delivery_id: prefix,
        delivery_name: String::new(),
    })
}

pub fn determine_delivery_override_from_raw_info(
    tracking_number: &str,
    raw_delivery_product_info: &Value,
) -> Option<DeliveryOverride> {
    let delivery_id = raw_delivery_product_info
        .get("deliveryId")
        .and_then(Value::as_str)
        .unwrap_or("");
    determine_delivery_override_on_mismatch(
        tracking_number,
        &DeliveryProductInfo {
            delivery_id: delivery_id.to_string(),
            delivery_name: raw_delivery_product_info
                .get("deliveryName")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            ..DeliveryProductInfo::default()
        },
    )
}

/// 补发货 reason = 2（商品拆分包裹）。
pub const COMPENSATION_REASON_SPLIT_PACKAGE: i64 = 2;

/// 这些错误码表示订单不支持补发货，应降级到改物流（updateDeliveryInfo）。
const COMPENSATION_FALLBACK_CODES: [i64; 4] = [
    6060494, // 商品未完成发货
    6060495, // 订单超出补发上限
    6060497, // 订单不支持补发
    6060499, // 商品不支持补发
];

pub fn is_compensation_fallback_error(code: i64) -> bool {
    COMPENSATION_FALLBACK_CODES.contains(&code)
}

pub fn build_compensation_delivery_payload(
    order_id: &str,
    waybill_id: &str,
    delivery_id: &str,
    reason: i64,
    product_infos: &[DeliveryProductItem],
) -> Value {
    let items: Vec<Value> = product_infos
        .iter()
        .map(|p| {
            serde_json::json!({
                "productId": p.product_id,
                "skuId": p.sku_id,
                "productCnt": p.product_cnt,
            })
        })
        .collect();

    serde_json::json!({
        "orderId": order_id.trim(),
        "reason": reason,
        "deliveryProductInfo": [{
            "deliveryId": delivery_id,
            "waybillId": waybill_id.trim(),
            "productInfos": items,
        }],
    })
}

pub fn delivery_update_succeeded(result: &Value) -> bool {
    result.get("success").and_then(Value::as_bool) == Some(true)
        || (result.get("code").and_then(Value::as_i64) == Some(0)
            && result.get("errcode").and_then(Value::as_i64) == Some(0))
        || (result.get("errcode").is_none()
            && result.get("ret").and_then(Value::as_i64) == Some(0)
            && matches!(result.get("code").and_then(Value::as_i64), Some(0) | None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_raw_delivery_info(delivery_id: &str, waybill_id: &str) -> DeliveryProductInfo {
        DeliveryProductInfo {
            delivery_id: delivery_id.into(),
            delivery_name: if delivery_id == "ZTO" {
                "中通快递".into()
            } else {
                "极兔速递".into()
            },
            waybill_id: waybill_id.into(),
            deliver_type: Some(1),
            waybill_status: Some(2),
            is_all_product: Some(false),
            delivery_time: Some("1775641487".into()),
            product_infos: vec![DeliveryProductItem {
                product_id: "10000496403296".into(),
                sku_id: "7982968968".into(),
                product_cnt: 1,
            }],
        }
    }

    #[test]
    fn build_update_delivery_payload_keeps_old_and_new_objects() {
        let raw_info = make_raw_delivery_info("ZTO", "73666162791371");
        let payload =
            build_update_delivery_payload("3735560095122745088", "77777777777777", &raw_info, None);
        assert_eq!(payload.order_id, "3735560095122745088");
        assert_eq!(payload.change_info[0].old.delivery_id, "ZTO");
        assert_eq!(payload.change_info[0].new.delivery_id, "ZTO");
        assert_eq!(payload.change_info[0].new.waybill_id, "77777777777777");
        assert_eq!(payload.change_info[0].old.waybill_id, "73666162791371");
    }

    #[test]
    fn build_update_delivery_payload_can_override_new_fields_only() {
        let raw_info = make_raw_delivery_info("ZTO", "73666162791371");
        let payload = build_update_delivery_payload(
            "3735560095122745088",
            "JT1234567890",
            &raw_info,
            Some(&DeliveryOverride {
                delivery_id: "JT".into(),
                delivery_name: "极兔速递".into(),
            }),
        );
        assert_eq!(payload.change_info[0].old.delivery_id, "ZTO");
        assert_eq!(payload.change_info[0].new.delivery_id, "JT");
        assert_eq!(payload.change_info[0].new.delivery_name, "极兔速递");
    }

    #[test]
    fn extract_delivery_snapshot_requires_delivery_and_products() {
        let raw_info = make_raw_delivery_info("ZTO", "73666162791371");
        let snapshot = extract_delivery_snapshot(&raw_info).unwrap();
        assert_eq!(snapshot.delivery_id, "ZTO");
        assert_eq!(snapshot.product_infos[0].sku_id, "7982968968");
    }

    #[test]
    fn build_delivery_candidates_prefers_original_then_tracking_prefix() {
        let snapshot = DeliverySnapshot {
            delivery_id: "ZTO".into(),
            delivery_name: "中通快递".into(),
            waybill_id: "old".into(),
            product_infos: vec![DeliveryProductItem {
                product_id: "1".into(),
                sku_id: "2".into(),
                product_cnt: 1,
            }],
        };
        let candidates = build_delivery_candidates("JT1234567890", &snapshot);
        assert_eq!(candidates[0].delivery_id, "ZTO");
        assert_eq!(candidates[1].delivery_id, "JT");
    }

    #[test]
    fn update_result_accepts_success_variants() {
        assert!(delivery_update_succeeded(&json!({"code": 0, "errcode": 0})));
        assert!(delivery_update_succeeded(&json!({"ret": 0, "code": 0})));
        assert!(delivery_update_succeeded(&json!({"success": true})));
        assert!(!delivery_update_succeeded(
            &json!({"success": false, "errmsg": "bad"})
        ));
    }

    #[test]
    fn mismatch_retry_uses_tracking_prefix_only_when_changed() {
        let raw_info = make_raw_delivery_info("ZTO", "73666162791371");
        let override_info = determine_delivery_override_on_mismatch("JT0001", &raw_info).unwrap();
        assert_eq!(override_info.delivery_id, "JT");
        assert!(override_info.delivery_name.is_empty());
        assert!(is_delivery_mismatch_error(&format!(
            "更新物流信息失败：{DELIVERY_MISMATCH_MESSAGE}"
        )));
        let same_prefix_info = make_raw_delivery_info("JT", "73666162791371");
        assert!(determine_delivery_override_on_mismatch("JT0001", &same_prefix_info).is_none());
    }

    #[test]
    fn confirmed_receipt_error_is_non_retryable() {
        assert!(is_non_retryable_delivery_error(
            "更新物流信息失败：订单已确认收货，不支持修改物流"
        ));
        assert!(!is_non_retryable_delivery_error(
            "更新物流信息失败：快递单号与所选物流商不匹配"
        ));
    }

    #[test]
    fn raw_delivery_info_can_drive_auto_downgrade_mapping() {
        let raw = json!({
            "deliveryId": "ZTO",
            "deliveryName": "中通快递",
            "waybillId": "73666162791371"
        });
        let override_info =
            determine_delivery_override_from_raw_info("  SF000123456  ", &raw).unwrap();
        assert_eq!(override_info.delivery_id, "SF");
        assert_eq!(override_info.delivery_name, "");

        let same_prefix = json!({"deliveryId": "SF", "deliveryName": "顺丰速运"});
        assert!(determine_delivery_override_from_raw_info("SF000123456", &same_prefix).is_none());
    }

    #[test]
    fn compensation_fallback_codes_are_recognized() {
        assert!(is_compensation_fallback_error(6060494));
        assert!(is_compensation_fallback_error(6060495));
        assert!(is_compensation_fallback_error(6060497));
        assert!(is_compensation_fallback_error(6060499));
        assert!(!is_compensation_fallback_error(0));
        assert!(!is_compensation_fallback_error(6060479));
    }

    #[test]
    fn build_compensation_payload_shapes_correctly() {
        let infos = vec![DeliveryProductItem {
            product_id: "P1".into(),
            sku_id: "S1".into(),
            product_cnt: 2,
        }];
        let payload = build_compensation_delivery_payload("ORD-1", "  WB-123  ", "ZTO", 2, &infos);
        assert_eq!(payload["orderId"], "ORD-1");
        assert_eq!(payload["reason"], 2);
        assert_eq!(payload["deliveryProductInfo"][0]["deliveryId"], "ZTO");
        assert_eq!(payload["deliveryProductInfo"][0]["waybillId"], "WB-123");
        assert_eq!(
            payload["deliveryProductInfo"][0]["productInfos"][0]["productId"],
            "P1"
        );
    }

    #[test]
    fn build_raw_update_payload_only_changes_waybill_or_delivery_override() {
        let raw = json!({
            "deliveryId": "ZTO",
            "deliveryName": "中通快递",
            "waybillId": "73666162791371",
            "deliverType": 1,
            "waybillStatus": 2,
            "extInfo": {
                "batchNo": "B-1",
                "packages": [{"skuId": "7982968968", "count": 1}]
            }
        });

        let payload = build_raw_update_delivery_payload(
            "3735560095122745088",
            "  SF1234567890  ",
            &raw,
            Some(&DeliveryOverride {
                delivery_id: "SF".into(),
                delivery_name: "顺丰速运".into(),
            }),
        )
        .expect("payload");

        assert_eq!(payload["orderId"], "3735560095122745088");
        assert_eq!(payload["changeInfo"][0]["old"], raw);
        assert_eq!(payload["changeInfo"][0]["new"]["waybillId"], "SF1234567890");
        assert_eq!(payload["changeInfo"][0]["new"]["deliveryId"], "SF");
        assert_eq!(payload["changeInfo"][0]["new"]["deliveryName"], "顺丰速运");
        assert_eq!(payload["changeInfo"][0]["new"]["extInfo"], raw["extInfo"]);
    }
}
