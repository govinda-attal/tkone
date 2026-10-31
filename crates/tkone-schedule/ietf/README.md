# The tkone schedule format as an Internet-Draft

This folder holds the grammar of the schedule format in ABNF, and an Internet-Draft built from
it and from [GRAMMAR.md](../GRAMMAR.md).

| File | What it is | Edit it? |
|------|------------|----------|
| `tkone-schedule.abnf` | The grammar, in ABNF (RFC 5234 with RFC 7405). The single source: `GRAMMAR.md` and the draft copy it. | yes |
| `draft.template.md` | The draft's IETF framing: front matter, abstract, introduction, conventions, security and IANA sections, references. | yes |
| `build.py` | Assembles and checks everything. | yes |
| `draft-attal-tkone-schedule-00.md` | The draft, in [kramdown-rfc](https://github.com/cabo/kramdown-rfc) Markdown. **Generated.** | no |

The rules and examples in the draft come from `GRAMMAR.md` sections 1–4, so the same text is
checked three ways:

- `cargo test` (`src/grammar_tests.rs`): every schedule in GRAMMAR.md's accepted and rejected
  tables is built with the Rust library.
- `build.py --validate`: the accepted examples match the ABNF, and the syntactically rejected
  ones don't.
- `build.py --check`: GRAMMAR.md's ABNF blocks and the generated draft are up to date.

## Workflow

After changing the ABNF, `GRAMMAR.md` or the template:

```sh
python3 ietf/build.py            # regenerate GRAMMAR.md's ABNF blocks and the draft
python3 ietf/build.py --check --validate
cargo test -p tkone-schedule grammar
```

CI runs the same checks (`.github/workflows/spec.yml`) and fails if anything is out of date.

In `GRAMMAR.md`, the ABNF blocks sit between `<!-- abnf:NAME -->` and `<!-- /abnf -->` markers,
where `NAME` is a `; @section NAME` of the ABNF file. Text between `<!-- not-in-draft -->` and
`<!-- /not-in-draft -->` stays out of the draft.

## Tools

| Tool | Needed for | Install |
|------|------------|---------|
| Python 3.10+ | everything | — |
| `abnf` | `--validate` | `pip install -r ietf/requirements.txt` |
| `xml2rfc` | `--render` | `pip install -r ietf/requirements.txt` |
| `kramdown-rfc` | `--render` | `gem install kramdown-rfc` (needs Ruby 3.2 or later) |

macOS ships Ruby 2.6, which current `kramdown-rfc` does not support. Either install a newer Ruby
(for example `brew install ruby`), or pin older dependencies:

```sh
gem install connection_pool -v 2.4.1
gem install net-http-persistent -v 4.0.2
gem install json_pure -v 2.6.3
gem install kramdown-rfc -v '~> 1.6.0'
```

Rendering fetches the RFC reference entries from bib.ietf.org, so it needs network access. The
rendered `.xml`, `.txt` and `.html` files are not committed.

## Before submitting

- Add an email address for the author in `draft.template.md`.
- Bump `docname` (`-00`, `-01`, …) for each new submission; `build.py` names the output after it.
- Check the result with the [IETF idnits tool](https://author-tools.ietf.org/idnits).
