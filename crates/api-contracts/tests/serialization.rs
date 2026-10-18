use memedock_api_contracts::{ApiEnvelope, ApiMeta, ChangePasswordRequest, DeviceInfo};

#[test]
fn wire_contract_uses_camel_case_and_explicit_nulls() -> Result<(), Box<dyn std::error::Error>> {
    let value = serde_json::to_value(ApiEnvelope {
        code: 0,
        biz_err: "OK".into(),
        msg: "success".into(),
        data: DeviceInfo {
            id: "device".into(),
            name: "Phone".into(),
            created_at: "now".into(),
            revoked_at: None,
            last_seen_at: None,
        },
        meta: ApiMeta {
            request_id: "request".into(),
            timestamp: "now".into(),
        },
    })?;
    assert_eq!(value["bizErr"], "OK");
    assert_eq!(value["meta"]["requestId"], "request");
    assert!(value["data"]["revokedAt"].is_null());
    assert!(
        serde_json::from_str::<ChangePasswordRequest>(
            r#"{"oldPassword":"a","newPassword":"b","extra":true}"#
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn sync_requests_preserve_validated_domain_contracts_and_reject_extra_fields()
-> Result<(), Box<dyn std::error::Error>> {
    use memedock_domain::{
        change::{Operation, OperationKind},
        identity::{DeviceId, LibraryId, OperationId, SyncEpoch, TagId},
        sync::OperationEnvelope,
        version::TimestampMs,
    };
    let request = memedock_api_contracts::PushRequest {
        operation: OperationEnvelope::new(
            LibraryId::new(),
            SyncEpoch::new(),
            DeviceId::new(),
            OperationId::new(),
            TimestampMs::new(42),
            Operation::new(OperationKind::CreateTag {
                tag_id: TagId::new(),
                name: "tag".parse()?,
            })?,
        ),
    };
    let mut wire = serde_json::to_value(&request)?;
    assert_eq!(wire["operation"]["protocol_version"], 1);
    let decoded: memedock_api_contracts::PushRequest = serde_json::from_value(wire.clone())?;
    assert_eq!(decoded.operation, request.operation);
    wire["operation"]["protocol_version"] = serde_json::json!(2);
    assert!(serde_json::from_value::<memedock_api_contracts::PushRequest>(wire).is_err());
    let mut wire = serde_json::to_value(&request)?;
    wire["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<memedock_api_contracts::PushRequest>(wire).is_err());
    Ok(())
}
