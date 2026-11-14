# L302 — Tonic Gating Mathematical Specification

**Status:** Quarantined Experimental Research Lane (Feature Flag: `INOS_ENABLE_TONIC_GATING`)  
**Scope:** Mathematical definition, dynamical state equations, calibration thresholds, and safety invariants.

---

## 1. Theoretical Foundation

In biological neural circuits, "tonic" gating refers to sustained, slow-modulating baseline conductance (in contrast to phasic, fast transient spikes) that regulates whether downstream cortical regions are receptive or inhibited.

In the Intent-Native OS architecture, **Tonic Gating** modulates execution depth based on a continuous state vector combining intent ambiguity, semantic consequence risk, model confidence, and system resource pressure.

Rather than invoking an expensive dual-model arbitration pipeline on every keystroke or command, the tonic gate maintains an adaptive state variable $T_t \in [0.0, 1.0]$ that determines:
- **Low Tone ($T_t < \theta_{\text{fast}}$):** Bypass neural models entirely; route to the sub-millisecond deterministic compiler.
- **Moderate Tone ($\theta_{\text{fast}} \le T_t < \theta_{\text{arbitration}}$):** Route to single fast model (Model A) for lightweight parsing.
- **High Tone ($T_t \ge \theta_{\text{arbitration}}$):** Trigger full dual-model arbitration (Model A $\to$ Model B $\to$ Deterministic Verifier).

Crucially, **Tonic Gating is strictly an efficiency and routing optimization; it possesses zero authority to grant execution rights.**

---

## 2. Dynamical State Equations

Let $T_t \in [0.0, 1.0]$ be the scalar tonic activation at step $t$.

### 2.1 State Evolution Equation

$$T_t = \lambda T_{t-1} + (1 - \lambda) \cdot \sigma\Big( \mathbf{w}^T \mathbf{\Phi}(x_t) - b \Big)$$

where:
- $\lambda \in [0.0, 1.0)$ is the momentum / decay factor (default: $\lambda = 0.35$).
- $\sigma(z) = \frac{1}{1 + e^{-z}}$ is the logistic sigmoid function.
- $\mathbf{\Phi}(x_t) = [H_{\text{lex}}(x), C_{\text{risk}}(x), U_{\text{system}}]^T$ is the feature vector:
  1. $H_{\text{lex}}(x) \in [0.0, 1.0]$: Lexical ambiguity score (presence of vague quantifiers, e.g., "old", "anything important", "clean up", "stuff").
  2. $C_{\text{risk}}(x) \in [0.0, 1.0]$: Consequence tier of the intent domain (e.g., ReadOnly = 0.0, Create = 0.3, Modify = 0.7, Delete/Overwrite = 1.0).
  3. $U_{\text{system}} \in [0.0, 1.0]$: Current system resource load (RAM pressure, active background services).
- $\mathbf{w} = [w_{\text{ambiguity}}, w_{\text{risk}}, w_{\text{resource}}]^T$: Weight vector calibrated such that $w_{\text{risk}} > w_{\text{ambiguity}} > w_{\text{resource}}$.
- $b$: Basal threshold bias (default: $b = 1.20$).

### 2.2 Routing Thresholds

$$\text{Route}(T_t) = \begin{cases}
\text{PathDeterministic} & \text{if } T_t < \theta_{\text{fast}} \quad (\text{default: } 0.25) \\
\text{PathSingleModel}   & \text{if } \theta_{\text{fast}} \le T_t < \theta_{\text{arbitration}} \quad (\text{default: } 0.65) \\
\text{PathDualModelReview} & \text{if } T_t \ge \theta_{\text{arbitration}}
\end{cases}$$

---

## 3. Safety Invariants & Fail-Safe Defaults

1. **Monotonic Risk Elevation (No Suppressing Dangerous Operations):**  
   If $C_{\text{risk}}(x) \ge 0.8$ (destructive operations like delete, drop, overwrite), then regardless of lexical certainty, the tonic gate forces:
   $$T_t \ge \theta_{\text{arbitration}}$$
   Low tone can *never* suppress dual review for irreversible actions.

2. **Fail-Closed on Uncalibrated State:**  
   If the gate confidence score is low (e.g., input contains unmapped tokens, anomalous distributions, or uninitialized state), the gate defaults to:
   $$T_t = 1.0 \implies \text{Full Review \& Deterministic Verification}$$

3. **Inability to Bypass the Deterministic Verifier:**  
   Regardless of whether the gate selects deterministic, single-model, or dual-model execution, all output plans must pass the typed `DeterministicVerifier` before a capability token can be issued.
