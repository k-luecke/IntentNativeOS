# Local Model Optimization & Research Agenda (APF-001)

**Document Identifier:** `APF-001-LOCAL-INFERENCE-RESEARCH`  
**Execution Context:** 16 GiB Reference Hardware Envelope, Dual-Model Architecture, Freestanding C/Rust Runtime

---

## 1. Local Intelligence Principles & Invariants

1. **Local-First & Completely Offline:** Zero external API or cloud model dependencies. All reasoning, extraction, and synthesis run on local CPU/GPU hardware.
2. **Deterministic-First Routing:** Deterministic parsers, regex extractors, and typed command registries resolve exact inputs in microseconds with zero token consumption. A language model is invoked only when semantic ambiguity or natural language synthesis is genuinely required.
3. **Authority Is Not Intelligence:** Models produce unprivileged proposals (`AppCommandProposal`). They cannot authorize file mutations, alter capabilities, bypass permission checks, or elevate system privileges.
4. **Dual-Model Functional Roles:**
   - **Model A (Architect & Reviewer):** High-precision, conservative reasoning model responsible for intent clarification, multi-step plan decomposition, security boundary verification, and invariant checking.
   - **Model B (Implementer & Challenger):** Fast, execution-oriented model responsible for draft generation, structured data translation, code snippet formatting, and initial parameter mapping.
5. **Empirical Preregistration for Experimental Techniques:** SCP (Semantic Context Compression), TG (Tonic Gating), and NNC (Network-Native Computing) are research candidates evaluated against blinded empirical benchmarks. If an experimental technique does not yield statistically verified latency, accuracy, or memory improvements over standard baselines, it is rejected and pruned from the critical path.

---

## 2. Model Candidates & Quantization Evaluation

### 2.1 Model Architecture Selection

| Family / Architecture | Parameters | Context Window | Target Format | Primary Role | Memory Footprint |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Qwen 2.5 Coder / Instruct** | 0.5B / 1.5B | 32K | GGUF Q4_K_M / Q8_0 | Model B (Fast Ring 3 Implementer / Worker) | ~0.5 – 1.2 GB |
| **Qwen 2.5 Coder / Instruct** | 3B / 7B | 32K | GGUF Q4_K_M | Model A (Architect / Verifier / Gatekeeper) | ~2.2 – 4.5 GB |
| **Llama 3.2** | 1B / 3B | 128K | GGUF Q4_K_M | Cross-Application Text Synthesis & Extraction | ~0.9 – 2.1 GB |

### 2.2 Quantization & Memory Management
- **Q4_K_M Standard:** Default operating point providing near-FP16 perplexity with a ~4x reduction in memory footprint and high memory bandwidth efficiency on x86-64 CPUs.
- **KV Cache Optimization:** Paged KV cache allocation with 8-bit quantized cache entries (`Q8_0` or `Q4_0`) for long contexts, ensuring context expansion does not trigger OOM under the 16 GiB reference budget.
- **Zero-Copy Memory-Mapped Weights (`mmap`):** Model weights are memory-mapped directly from storage, allowing the operating system kernel to manage page eviction and fast worker task switching without duplicate buffer copies.

---

## 3. Dynamic LoRA Architecture & Task Specialization

Instead of maintaining multiple large monolithic models, APF-001 uses a unified base model augmented by small, domain-specialized Low-Rank Adaptation (LoRA) adapters (Rank \(r = 16\), \(\alpha = 32\)):

| Adapter ID | Specialization Focus | Triggering Conditions | Memory Delta |
| :--- | :--- | :--- | :---: |
| **`lora-grid`** | Spreadsheet formula syntax, DAG dependencies, CSV schemas | Grid application focus or tabular intent queries | ~25 MB |
| **`lora-write`** | Typography, document outlining, markdown/DOCX structures | Write application focus or document composition | ~25 MB |
| **`lora-mail`** | RFC 5322 MIME extraction, event extraction from messages | Mail application focus, meeting detection | ~20 MB |
| **`lora-plan`** | Temporal logic, RFC 5545 recurrence rules, conflict resolution | Plan/Calendar event scheduling, reminder logic | ~20 MB |
| **`lora-sec`** | Capability verification, boundary threat detection, injection defense | Plan validation gate before execution | ~30 MB |

### Adapter Swapping Mechanics
- LoRA adapters are loaded into Ring 3 memory-mapped buffers.
- Fast runtime weight delta addition: \(W_{\text{effective}} = W_0 + \frac{\alpha}{r} (B \times A)\).
- Swapping between adapters requires zero base-model reloading, executing in \(< 50 \text{ ms}\).

---

## 4. Empirical Evaluation Protocol for SCP and TG

### 4.1 Semantic Context Compression (SCP)
- **Hypothesis:** Compressing long historical conversation and document context into structured semantic anchor embeddings reduces KV-cache memory pressure while preserving factual retrieval accuracy.
- **Benchmark Metric:** Context memory usage (MB) vs. Factual Recall Rate (needle-in-a-haystack test suite).
- **Preregistered Gate:** SCP will only be incorporated into production if it achieves \(\ge 2\times\) context length extension with \(< 3\%\) degradation in parameter extraction accuracy.

### 4.2 Tonic Gating (TG)
- **Hypothesis:** Dynamic layer-skipping / early-exit gating conditioned on token confidence scores reduces inference latency for simple classification and extraction tokens.
- **Benchmark Metric:** Tokens per second (tok/s), Time-To-First-Token (TTFT), and output BLEU/accuracy compared to full-depth generation.
- **Preregistered Gate:** TG must achieve \(\ge 25\%\) inference speedup on standard structured extraction tasks without producing schema invalidations.

---

## 5. Structured Output & Grammar-Constrained Decoding

To ensure 100% type safety and prevent malformed commands, model decoding is constrained at the token level by strict context-free grammars (GBNF or native parser state machines):
1. **Typed Intent Grammar (`INTENT_GBNF`):** Forces the model to emit only valid serialized JSON or OCaml S-expressions conforming to `AppCommandProposal`.
2. **Schema Invariants:** Invalid tokens receive zero probability mass during sampling, eliminating syntax parsing errors entirely.
3. **Deterministic Verifier:** Every generated proposal passes through the OCaml type checker before being routed to the capability broker.

---

## 6. Standardized Benchmark Suites

| Benchmark Suite | Case Count | Evaluation Focus | Primary Pass Criteria |
| :--- | :---: | :--- | :--- |
| **`bench-extract`** | 150 | Extraction of dates, recipients, amounts from emails and PDFs | \(\ge 98\%\) exact field match |
| **`bench-formula`** | 200 | Natural language query to correct Excel/Grid formula syntax | \(\ge 96\%\) valid formula execution |
| **`bench-plan`** | 100 | Calendar scheduling, conflict resolution, recurring events | 100% conflict-free schedule placement |
| **`bench-workflow`**| 75 | Cross-application intent execution (Mail -> Grid -> Write) | 100% valid capability grant requests |
| **`bench-security`**| 100 | Adversarial prompts, jailbreaks, unauthorized access attempts | 0.0% false authorization rate |
