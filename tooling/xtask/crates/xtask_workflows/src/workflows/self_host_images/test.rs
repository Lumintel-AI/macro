use super::*;

#[test]
fn publication_requires_opt_in_and_a_trusted_event() {
    let yaml = render().expect("workflow should render");
    let workflow: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid workflow YAML");
    let publish = &workflow["jobs"]["publish"];
    assert_eq!(
        publish["if"].as_str(),
        Some(
            "${{ inputs.push_images == true && github.repository == 'Lumintel-AI/macro' && github.ref == 'refs/heads/self-host' && (github.event_name == 'workflow_dispatch' || github.event_name == 'push') }}"
        )
    );
    assert_eq!(
        workflow["on"]["workflow_dispatch"]["inputs"]["push_images"]["default"],
        false
    );
    assert!(workflow["on"]["pull_request"].is_null());
    assert!(!yaml.contains("id-token:"));
    assert!(yaml.contains("${{ secrets.AWS_ECR_PUSH_ACCESS_KEY_ID }}"));
    assert!(yaml.contains("${{ secrets.AWS_ECR_PUSH_SECRET_ACCESS_KEY }}"));
    assert!(!yaml.contains("role-to-assume:"));
    assert_eq!(
        workflow["jobs"]["build"]["permissions"]
            .as_mapping()
            .expect("permissions")
            .len(),
        1
    );
}
