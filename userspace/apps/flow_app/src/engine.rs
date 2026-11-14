// userspace/apps/flow_app/src/engine.rs
// ============================================================================
// Native Ring 3 Flow Application — Generalized Application Automation ABI
// (`INOS-AUTOMATION-ABI-001`) & Multi-App Pipeline Orchestrator (`#![no_std]`)
// ============================================================================

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiType {
    String,
    U32,
    Float,
    Blob,
    DocRef,
}

#[derive(Debug, Clone, Copy)]
pub struct AbiParam {
    pub name: &'static str,
    pub ptype: AbiType,
}

#[derive(Debug, Clone, Copy)]
pub struct AbiOperation {
    pub app_id: &'static str,
    pub target_pid: u64,
    pub op_name: &'static str,
    pub target_output: &'static str,
    pub required_rights: u32,
    pub params: &'static [AbiParam],
    pub ret_type: AbiType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAppStage {
    NotesObserve,
    PlanNormalizeGeometry,
    CanvasVectorTransform,
    CalcCoordinateMath,
    GridRelationalLookup,
    WriteRecolorLegend,
    IntScriptRuleVm,
    ArchiveMdlVerify,
}

#[derive(Debug, Clone, Copy)]
pub struct FlowPipelinePlan {
    pub stages: [NativeAppStage; 6],
    pub stage_count: usize,
    pub requires_geometry_norm: bool,
    pub requires_silhouette_fill: bool,
    pub requires_recolor_pass: bool,
}

impl FlowPipelinePlan {
    pub const fn new() -> Self {
        Self {
            stages: [NativeAppStage::NotesObserve; 6],
            stage_count: 0,
            requires_geometry_norm: false,
            requires_silhouette_fill: false,
            requires_recolor_pass: false,
        }
    }

    pub fn push_stage(&mut self, stage: NativeAppStage) {
        if self.stage_count < 6 {
            self.stages[self.stage_count] = stage;
            self.stage_count += 1;
        }
    }
}
