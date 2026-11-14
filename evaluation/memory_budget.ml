(* 16 GiB Reference RAM Budget and Memory Accountant (Q005) *)
open Types

let reference_16gb_budget : memory_allocation_breakdown = {
  total_hardware_ram_mb = 16384;      (* 16 GiB total physical reference *)
  model_a_weights_mb = 6144;          (* Model A (Architect/Planner 7B Q4_K_M) *)
  model_b_weights_mb = 4096;          (* Model B (Implementer/Reviewer 3.8B Q4_K_M) *)
  kv_cache_context_mb = 1536;         (* Shared sliding-window KV cache *)
  kernel_and_paging_mb = 256;         (* Rust microkernel, page tables, DMA *)
  gui_compositor_mb = 512;            (* Native desktop compositor & surfaces *)
  guest_capsule_reserve_mb = 1024;    (* Ephemeral guest VM allocation reserve *)
  free_headroom_mb = 2816;            (* Uncommitted dynamic heap headroom *)
}

let validate_memory_budget (b : memory_allocation_breakdown) : (unit, string) result =
  let committed =
    b.model_a_weights_mb +
    b.model_b_weights_mb +
    b.kv_cache_context_mb +
    b.kernel_and_paging_mb +
    b.gui_compositor_mb +
    b.guest_capsule_reserve_mb +
    b.free_headroom_mb
  in
  if committed <> b.total_hardware_ram_mb then
    Error (Printf.sprintf "Memory budget mismatch: sum of partitions (%d MB) does not equal total hardware RAM (%d MB)"
             committed b.total_hardware_ram_mb)
  else if b.free_headroom_mb < 2048 then
    Error (Printf.sprintf "Insufficient free headroom: %d MB is less than minimum 2048 MB required for safety"
             b.free_headroom_mb)
  else
    Ok ()

let format_memory_report (b : memory_allocation_breakdown) : string =
  Printf.sprintf
    "=== 16 GiB Reference Memory Allocation Report ===\n\
     Total Physical RAM:   %5d MB (100.0%%)\n\
     Model A Weights:      %5d MB (%4.1f%%)\n\
     Model B Weights:      %5d MB (%4.1f%%)\n\
     KV Cache Context:     %5d MB (%4.1f%%)\n\
     OS Kernel & Paging:   %5d MB (%4.1f%%)\n\
     GUI & Compositor:     %5d MB (%4.1f%%)\n\
     Guest Capsule RAM:    %5d MB (%4.1f%%)\n\
     Free Headroom:        %5d MB (%4.1f%%)\n\
     ================================================"
    b.total_hardware_ram_mb
    b.model_a_weights_mb ((float_of_int b.model_a_weights_mb /. float_of_int b.total_hardware_ram_mb) *. 100.0)
    b.model_b_weights_mb ((float_of_int b.model_b_weights_mb /. float_of_int b.total_hardware_ram_mb) *. 100.0)
    b.kv_cache_context_mb ((float_of_int b.kv_cache_context_mb /. float_of_int b.total_hardware_ram_mb) *. 100.0)
    b.kernel_and_paging_mb ((float_of_int b.kernel_and_paging_mb /. float_of_int b.total_hardware_ram_mb) *. 100.0)
    b.gui_compositor_mb ((float_of_int b.gui_compositor_mb /. float_of_int b.total_hardware_ram_mb) *. 100.0)
    b.guest_capsule_reserve_mb ((float_of_int b.guest_capsule_reserve_mb /. float_of_int b.total_hardware_ram_mb) *. 100.0)
    b.free_headroom_mb ((float_of_int b.free_headroom_mb /. float_of_int b.total_hardware_ram_mb) *. 100.0)
