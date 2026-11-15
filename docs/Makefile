.PHONY: all clean
all: rdigital_rf-api.pdf
rdigital_rf-api.pdf: rdigital_rf-api.tex ../examples/stream_read.rs ../examples/gap_read.rs ../examples/raw_ring.rs
	pdflatex -interaction=nonstopmode -halt-on-error rdigital_rf-api.tex
	pdflatex -interaction=nonstopmode -halt-on-error rdigital_rf-api.tex
clean:
	rm -f rdigital_rf-api.aux rdigital_rf-api.log rdigital_rf-api.out
