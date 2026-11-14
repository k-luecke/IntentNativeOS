# INOS Native Browser: WHATWG Parser Conformance & Security Policy Matrix

**Document ID:** `WEB-02C-CONF-001`  
**Milestone:** `WEB-02C`  
**Specification Baseline:** WHATWG HTML Living Standard (Parsing Section)  
**Security Reference:** INOS SDU-01 Standards Register / Sentinel S103  
**Status:** Canonical Engineering Reference  

---

## 1. Scope & Purpose

This Conformance Matrix establishes the formal relationship between the INOS Native Browser HTML parser (`web_render`) and the WHATWG HTML parsing specification. 

INOS distinguishes between three distinct architectural categories:
1. **Conformant Features**: Constructs implemented strictly to the WHATWG specification.
2. **Unsupported Features & Expected Failures (XFAIL)**: Standard features intentionally omitted or deferred in this bare-metal, native Ring 3 implementation.
3. **Deliberate Security Policies**: Hard operational bounds enforced for memory containment and denial-of-service prevention. These are explicitly **product security policies**, not standard conformance omissions.

---

## 2. Deliberate INOS Security Policies vs. WHATWG Conformance

| Policy / Resource Constraint | INOS Limit | Rationale & Security Boundary | WHATWG Standard Behavior |
| :--- | :---: | :--- | :--- |
| **Max Payload Size** | `512 KiB` | Prevents Ring 3 heap exhaustion under unbounded payloads. Enforced at stream decoder boundary. | Unbounded stream processing. |
| **Max DOM Nodes** | `2,048 nodes` | Pre-allocated bounded arena size preventing heap fragmentation. | Unbounded node allocation. |
| **Max Nesting Depth** | `64 levels` | Prevents deep tag recursion and call stack exhaustion (`LimitExceeded(TreeDepth)`). | Unbounded stack of open elements. |
| **Max Attribute Value** | `1,024 bytes` | Prevents string allocation attacks in attribute tokenization (`LimitExceeded(AttrValueLength)`). | Unbounded attribute values. |
| **Max Attribute Name** | `64 bytes` | Bound on tag and attribute identifier length. | Unbounded tag/attribute names. |
| **Max Attributes / Tag** | `32 attributes`| Prevents quadratic attribute scanning attacks. | Unbounded attribute count. |
| **Max Work Operations** | `100,000 ops` | Deterministic computation budget against algorithmic complexity attacks. | Unbounded CPU consumption. |
| **Inert Active Content** | Pure Text Nodes | `<script>` and `<style>` contents are stored purely as inert strings; zero JavaScript/CSS execution. | Dynamic script evaluation / CSS engine. |
| **Passive Event Handlers**| Pure Strings | `on*` attributes (`onclick`, `onload`) are stored as static strings; no execution runtime exists. | Script dispatch upon event trigger. |

---

## 3. Tokenizer Conformance Register

| Tokenizer State / Feature | WHATWG Reference | Status | Notes / Provenance |
| :--- | :--- | :---: | :--- |
| **Data State** | §13.2.5.1 | Conformant | Transitions to `TagOpen` on `<`, `CharacterReference` on `&`. |
| **Character Reference Decoding** | §13.2.5.72 | Conformant | Resolves `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, numeric decimal `&#123;`, numeric hex `&#x1F;`. |
| **Broken Entity Recovery** | §13.2.5.72 | Conformant | Unrecognized entities (e.g. `&unknown;`) emitted literally as characters. |
| **Tag Open / End Tag Open** | §13.2.5.6-7 | Conformant | Case-insensitive ASCII tag name scanning. |
| **Markup Declaration Open** | §13.2.5.42 | Conformant | Directs to `<!--` comment or `<!DOCTYPE`. |
| **Attribute Name / Value** | §13.2.5.32-38| Conformant | Double-quoted, single-quoted, and unquoted attribute parsing. |
| **Attribute Deduplication** | §13.2.5.34 | Conformant | First attribute instance wins; duplicate attributes ignored per WHATWG. |
| **Self-Closing Flag** | §13.2.5.40 | Conformant | Acknowledged on void and foreign elements. |
| **Bogus Comments** | §13.2.5.41 | Conformant | `<?...>` and invalid declarations consumed safely as comments. |
| **Raw Text Parsing** | §13.2.5.3 | Conformant | `<script>` and `<style>` consume raw characters until matching closing tag. |
| **CDATASect** | §13.2.5.64 | Unsupported (XFAIL) | XML/SVG CDATA blocks treated as comments/text in HTML mode. |
| **RCDATA Elements** | §13.2.5.2 | Unsupported (XFAIL) | `<textarea>`, `<title>` parsed with standard tokenizer data states. |

---

## 4. Tree Construction Conformance Register

| Tree Construction Rule | WHATWG Reference | Status | Notes / Provenance |
| :--- | :--- | :---: | :--- |
| **Doctype Handling** | §13.2.6.4.1 | Conformant | Initial doctype recorded in DOM arena; subsequent doctypes ignored. |
| **Implicit `<html>` Generation** | §13.2.6.4.2 | Conformant | Automatically synthesizes root `<html>` if missing. |
| **Implicit `<head>` Generation** | §13.2.6.4.3 | Conformant | Automatically synthesizes `<head>` on head elements. |
| **Implicit `<body>` Generation** | §13.2.6.4.4 | Conformant | Transitions to `InBody` on body content or explicit `<body>`. |
| **Implicit Paragraph Closing** | §13.2.6.4.7 | Conformant | Block elements (`div`, `p`, `h1..h6`, `ul`, `ol`, `table`, etc.) pop open `<p>`. |
| **List Item Closing** | §13.2.6.4.7 | Conformant | Encountering `<li>` closes preceding open `<li>`. |
| **Table Structure Rules** | §13.2.6.4.9 | Conformant | `<tr>` pops preceding `<tr>`; `<td>`/`<th>` pops preceding cell; `</table>` closes table children. |
| **Void Elements** | §13.1.2 | Conformant | `area, base, br, col, embed, hr, img, input, link, meta, param, source, track, wbr` self-close without open-element stack push. |
| **Text Node Coalescing** | §13.2.6 | Conformant | Adjacent character tokens merged into contiguous text nodes. |
| **Adoption Agency Algorithm**| §13.2.6.4.7 | Unsupported (XFAIL) | Complex misnested formatting tags (e.g. `<b><i></b></i>`) use stack truncation rather than AAA. |
| **Foster Parenting** | §13.2.6.1 | Unsupported (XFAIL) | Stray table content placed in current node rather than foster parented before table. |
| **Foreign Content (SVG/MathML)**| §13.2.6.5 | Unsupported (XFAIL) | Foreign elements parsed as standard generic HTML element nodes. |
| **Framesets** | §13.2.6.4.18 | Unsupported (XFAIL) | Deprecated `<frameset>` and `<frame>` treated as generic elements. |

---

## 5. Upstream Test Fixture Provenance & Expected Failures

The test suite in `tests/conformance/fixtures.rs` uses test vectors derived from upstream **`html5lib-tests`** (commit `a9bc38e`):
1. **`tokenizer/entities.test`**: Entity resolution across standard and edge-case character references.
2. **`tree-construction/tests1.dat`**: Basic paragraph, heading, list, and table structure tests.
3. **`tree-construction/tests2.dat`**: Void element insertion and text coalescing.

### Expected Failures Register (XFAIL)
- **`XFAIL-001`**: AAA Misnested Formatting (`<b>1<i>2</b>3</i>`): Produces flattened structure instead of reconstructed `<i>` node.
- **`XFAIL-002`**: Foster Parenting in Tables (`<table>1<tr><td>2</td></tr></table>`): Text `1` appended to `table` instead of before `table`.
- **`XFAIL-003`**: SVG Foreign Attributes (`<svg viewBox="...">`): Preserves case as lowercase rather than camelCase.
