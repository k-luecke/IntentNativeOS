# L301 — Structured Capsule Projection (SCP) Mathematical Specification

**Status:** Quarantined Experimental Research Lane (Feature Flag: `INOS_ENABLE_SCP`)  
**Scope:** Mathematical definition, interface contract, and failure hypotheses for constrained decoding via subspace projection.

---

## 1. Theoretical Foundation

Large Language Models autoregressively sample tokens from an unconstrained probability distribution over vocabulary $V$:
$$P(y_t \mid y_{<t}, x) = \text{softmax}(z_t)$$
where $z_t \in \mathbb{R}^{|V|}$ is the unnormalized logit vector produced by the final linear projection layer.

In an operating system context where intent representations must strictly conform to a typed Abstract Syntax Tree (AST) or structured capsule schema $C \subset V^*$, unconstrained sampling risks generating syntactically invalid, ill-formed, or capability-violating tokens.

**Structured Capsule Projection (SCP)** is defined as an operator $\Pi_{\mathcal{K}(s)}$ that projects the model logit vector $z_t$ onto an admissible logit subspace constrained by the current parsing state $s \in \mathcal{S}$:

$$\tilde{z}_t = \Pi_{\mathcal{K}(s)}(z_t)$$

where $\mathcal{K}(s) \subseteq V$ is the set of legally admissible next tokens determined by the deterministic grammar automaton at state $s$.

---

## 2. Mathematical Definition

### 2.1 Masked Simplex Projection

Let $\mathcal{M}(s) \in \{-\infty, 0\}^{|V|}$ be the semantic constraint mask vector:
$$\mathcal{M}_i(s) = \begin{cases} 
0 & \text{if } i \in \mathcal{K}(s) \\
-\infty & \text{if } i \notin \mathcal{K}(s)
\end{cases}$$

The projected logits $\tilde{z}_t$ are computed element-wise:
$$\tilde{z}_{t, i} = z_{t, i} + \mathcal{M}_i(s)$$

Applying the softmax transformation yields the conditioned probability distribution:
$$\tilde{P}(y_t = i \mid y_{<t}, x) = \frac{\exp(\tilde{z}_{t, i})}{\sum_{j \in \mathcal{K}(s)} \exp(z_{t, j})}$$

### 2.2 Continuous Latent Embedding Projection (Optional Subspace Variant)

For intermediate latent states $h_t \in \mathbb{R}^d$ prior to output projection, let $W_C \in \mathbb{R}^{d \times k}$ represent an orthonormal basis spanning the semantic capsule subspace. The orthogonal projection is:
$$\hat{h}_t = W_C W_C^T h_t$$
with residual divergence metric:
$$\delta_{\text{divergence}} = \frac{\|h_t - \hat{h}_t\|_2}{\|h_t\|_2}$$

If $\delta_{\text{divergence}} > \epsilon_{\text{threshold}}$, the generation is flagged as out-of-capsule entropy excursion.

---

## 3. Pipeline Placement & Data Flow

```
+-------------------------------------------------------------+
|                      Inference Engine                       |
|                                                             |
|  [ Transformer Forward Pass ]                               |
|               |                                             |
|        Raw Logits z_t                                       |
|               |                                             |
|               v                                             |
|     +--------------------+                                  |
|     |  SCP Pre-Filter    |<--- Current Parser State s       |
|     |  (Masking / Simplex|     from Capsule Grammar DFA     |
|     |   Projection)      |                                  |
|     +--------------------+                                  |
|               |                                             |
|       Projected Logits z'_t                                 |
|               |                                             |
|               v                                             |
|     [ Greedy / Temp Sampler ]                               |
|               |                                             |
|          Token y_t                                          |
|               |                                             |
|               v                                             |
|     [ Grammar DFA Advance ]                                 |
|               |                                             |
|        New Parser State s'                                  |
+-------------------------------------------------------------+
```

---

## 4. Expected Effects vs. Failure Hypotheses

### Expected Benefits
1. **100% Syntactic Conformance:** Every generated sequence strictly conforms to the formal intent grammar DFA (no unclosed brackets, invalid keys, or malformed enum values).
2. **Zero In-Band Token Trapping:** By disallowing illegal transitions before sampling, the model cannot branch into dead-end parsing states.

### Failure Hypotheses (To Be Tested in L306/L307)
1. **Semantic Degeneration (Perplexity Explosion):** If the model's natural top-k predictions are all masked out because the grammar is overly restrictive, the model may be forced to emit nonsensical low-probability tokens that satisfy syntax but destroy semantic intent.
2. **Grammar Automaton Deadlock:** An edge case where $\mathcal{K}(s) = \emptyset$ terminates generation abruptly with an unparseable fragment.
3. **Latency Overhead:** Recomputing $\mathcal{K}(s)$ and updating $|V|$-sized masks at each decoding step adds latency that may exceed the cost of post-hoc verification.
4. **False Security Illusion:** Syntactic validity does *not* imply semantic safety. A syntactically perfect command can still specify dangerous arguments (`DELETE /`). SCP must never be treated as an authorization mechanism.
