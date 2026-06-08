# Удобный запуск одним файлом: `make` собирает release и кладёт в корень
# симлинк ./l0kus-messanger -> target/release/l0kus-messanger.

BIN := l0kus-messanger
TARGET := target/release/$(BIN)

.PHONY: all build link run clean

all: link

build:
	cargo build --release

# Собрать и создать/обновить симлинк в корне репозитория.
link: build
	ln -sf $(TARGET) ./$(BIN)
	@echo "Готово: ./$(BIN) — запускайте через ./$(BIN)"

# Собрать и сразу запустить TUI.
run: build
	./$(TARGET)

clean:
	cargo clean
	rm -f ./$(BIN)
