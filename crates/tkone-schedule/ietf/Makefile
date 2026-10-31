# Shortcuts for ietf/build.py. Run from this directory: `make check`.

PYTHON ?= python3

.PHONY: build check validate render tools

build:          ## regenerate GRAMMAR.md's ABNF blocks and the draft
	$(PYTHON) build.py

check:          ## fail if GRAMMAR.md or the draft is out of date, and validate the ABNF
	$(PYTHON) build.py --check --validate

validate:       ## check GRAMMAR.md's examples against the ABNF
	$(PYTHON) build.py --validate

render: build   ## render the draft to .xml, .txt and .html
	$(PYTHON) build.py --render

tools:          ## install the Python tools (kramdown-rfc is a Ruby gem: see README.md)
	$(PYTHON) -m pip install -r requirements.txt
