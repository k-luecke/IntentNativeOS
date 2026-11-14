(* tests/test_capsules.ml - Tests for Capsule Specification, Resolver, Broker, and Lifecycle (V003, V004, V009, V010, V011, V012, V014) *)

open Inos_capsules.Types
open Inos_capsules.Resolver
open Inos_capsules.Broker
open Inos_capsules.Lifecycle

let () =
  Printf.printf "=== Testing Capsule Specifications and Compatibility Resolver ===\n";

  (* 1. Test binary classifier (V012) *)
  let elf_header = Bytes.of_string "\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00" in
  let pe_header = Bytes.of_string "MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xff\xff\x00\x00" in

  let elf_res = classify_binary elf_header in
  assert (elf_res.format = NativeElf64);
  assert (not elf_res.requires_guest_vm);
  assert (elf_res.target_os = TargetNativeMicrokernel);

  let pe_res = classify_binary pe_header in
  assert (pe_res.format = WindowsPe32 || pe_res.format = WindowsPe32Plus);
  assert (pe_res.requires_guest_vm);

  (* 2. Test authority scope and device validation (V003) *)
  let scope = default_guest_scope ~network:false in
  let contract = {
    host_reserved_min_mb = 64;
    guest_requested_mb = 128;
    actual_allocated_mb = 128;
    reclaimable_on_exit = true;
  } in
  let instance = create_instance "capsule-win-01" scope contract in
  assert (validate_device_request instance DevVirtioConsole);
  assert (validate_device_request instance DevVirtioBlock);
  assert (not (validate_device_request instance (DevForbiddenDevice "RawPhysicalPCI")));

  (* 3. Test lifecycle and resource reclaim (V004, V011) *)
  assert (launch instance 404 = Ok ());
  assert (stop instance 0 = Ok ());
  let reclaimed_mb = reclaim instance in
  assert (reclaimed_mb = Ok 128);

  (* 4. Test file broker with document grants and clipboard isolation (V009, V010) *)
  let grants = [
    { relative_path = "document.docx"; read_allowed = true; write_allowed = true };
    { relative_path = "reference.pdf"; read_allowed = true; write_allowed = false };
  ] in
  let session = create_session "capsule-win-01" grants in
  assert (check_file_access session "document.docx" false);
  assert (check_file_access session "document.docx" true);
  assert (check_file_access session "reference.pdf" false);
  assert (not (check_file_access session "reference.pdf" true));
  assert (not (check_file_access session "../../../etc/shadow" false));
  assert (not (check_file_access session "C:\\Windows\\System32" true));

  (* Clipboard isolation *)
  assert (Result.is_error (attempt_clipboard_read session scope));
  assert (Result.is_error (attempt_clipboard_write session scope "stolen secret"));

  (* 5. Test lawful provisioning workflow verification (V014) *)
  let lawful_entitlement = {
    media_path = "/media/user/win_install.iso";
    license_hash = "SHA256_VALID_ENTITLEMENT_HASH";
    user_supplied_media = true;
    network_download_prohibited = true;
  } in
  assert (verify_lawful_provisioning lawful_entitlement = Ok true);

  let unlawful_download = {
    lawful_entitlement with
    user_supplied_media = false;
  } in
  assert (Result.is_error (verify_lawful_provisioning unlawful_download));

  Printf.printf "[PASS] inos_capsules specifications and policies verified 100%%\n"
