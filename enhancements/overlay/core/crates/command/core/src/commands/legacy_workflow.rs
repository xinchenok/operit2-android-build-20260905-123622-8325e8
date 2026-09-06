use crate::output::CoreCommandOutput;
use operit_runtime::core::application::OperitApplication::OperitApplication;
use operit_runtime::services::LegacyWorkflowService::LegacyWorkflowService;

pub fn run_workflow_command(
    application: &mut OperitApplication,
    args: &[String],
    output: &mut CoreCommandOutput,
) -> Result<(), String> {
    let action = args.first().ok_or("legacy-workflow requires an action")?;
    let payload = match args.get(1) {
        Some(value) => serde_json::from_str(value)
            .map_err(|e| format!("Invalid legacy-workflow payload: {e}"))?,
        None => serde_json::json!({}),
    };
    let result =
        LegacyWorkflowService::command(&application.toolPkgBridgeRuntime, action, payload)?;
    output.push_stdout_line(result.to_string());
    output.setJsonStdout(result);
    Ok(())
}
