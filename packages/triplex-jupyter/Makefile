# The widget's JS is apps/triplex-web built as a single file. `make bundle`
# builds it and copies it to where the Python package loads it from.

STATIC := src/triplex_jupyter/static

.PHONY: help bundle test

help:
	@echo "make bundle   build apps/triplex-web's widget bundle and copy it into the package"
	@echo "make test     pytest (needs 'make bundle' and 'pip install -e .[dev]' first)"

bundle:
	$(MAKE) -C ../../apps/triplex-web widget
	mkdir -p $(STATIC)
	cp ../../apps/triplex-web/www/dist-widget/widget.js ../../apps/triplex-web/www/dist-widget/style.css $(STATIC)/

test:
	python -m pytest tests
