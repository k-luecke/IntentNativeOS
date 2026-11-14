(* Manual vs Automatic Task Total Human Attention Measurement (Q003) *)
open Types

let create_attention_record
    ~(task_name : string)
    ~(manual_min : float)
    ~(prompt_min : float)
    ~(supervision_min : float)
    ~(review_min : float)
    ~(correction_min : float) : attention_metric =
  let auto_total = prompt_min +. supervision_min +. review_min +. correction_min in
  {
    task_name;
    manual_baseline_minutes = manual_min;
    automated_intent_minutes = auto_total;
    prompt_time_minutes = prompt_min;
    supervision_time_minutes = supervision_min;
    review_time_minutes = review_min;
    correction_time_minutes = correction_min;
  }

let benchmark_dataset : attention_metric list = [
  create_attention_record
    ~task_name:"System Log Differential Analysis & Audit Receipt"
    ~manual_min:45.0
    ~prompt_min:0.5
    ~supervision_min:1.0
    ~review_min:1.5
    ~correction_min:0.0;
  create_attention_record
    ~task_name:"Encrypted Backup Archiving with Capability Scoping"
    ~manual_min:30.0
    ~prompt_min:0.4
    ~supervision_min:0.8
    ~review_min:1.0
    ~correction_min:0.2;
  create_attention_record
    ~task_name:"Disposable Guest Capsule Provisioning and File Extraction"
    ~manual_min:25.0
    ~prompt_min:0.3
    ~supervision_min:0.5
    ~review_min:0.8
    ~correction_min:0.0;
]

let calculate_attention_reduction (m : attention_metric) : float =
  m.manual_baseline_minutes /. m.automated_intent_minutes
