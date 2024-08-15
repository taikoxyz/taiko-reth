# build/Makefile

.PHONY: install propose

install:
	./scripts/setup_deps.sh

propose:
	@if [ -z "$(number)" ]; then \
		echo "Error: 'number' parameter is required. Usage: make propose number=1"; \
		exit 1; \
	fi
	./scripts/propose_block.sh $(number)