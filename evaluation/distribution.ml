(* Developer Image Manifest, Build Instructions, and Known Risks (Q010) *)
open Types

let release_distribution_manifest : distribution_manifest = {
  image_name = "inos-preview-uefi-x86_64.efi";
  target_arch = "x86_64-unknown-uefi";
  sha256_hash = "613e568bf5111be394a54c302963d015bd75be4d9142e6305cb2f1bbfbe03153";
  file_size_bytes = 359936;
  build_reproducibility = "100% deterministic using Cargo 1.94+ and Dune 3.16+ in POSIX environment with zero Python";
  known_hardware_risks = [
    "Experimental Developer Preview: Strictly virtualized execution under QEMU recommended; bare metal not certified.";
    "Physical GPU passthrough is explicitly disabled to preserve microkernel capability isolation.";
    "Local model inference requires minimum 16 GiB physical RAM to avoid paging contention.";
    "Dual A/B update partition table requires minimum 2 GB virtual disk allocation.";
  ];
}

let generate_readme_instructions () : string =
  Printf.sprintf
    "# INOS Developer Image Quickstart\n\n\
     ## Build From Clean Environment\n\
     ```bash\n\
     # 1. Build kernel and user init\n\
     ./scripts/build_kernel.sh\n\n\
     # 2. Run automated QEMU verification\n\
     ./scripts/run_qemu_test.sh\n\
     ```\n\n\
     ## Checksums\n\
     - SHA-256: %s\n\
     - Binary: %s (%d bytes)\n"
    release_distribution_manifest.sha256_hash
    release_distribution_manifest.image_name
    release_distribution_manifest.file_size_bytes
