# Benchmark

Mede a análise (léxico, sintaxe e semântica) e a execução no interpretador.

```sh
python3 bench/gerar.py target/bench
cargo run --release -p qjo-core --example bench -- target/bench/analise_10000.qjo
```

Os números do README da raiz foram medidos assim, mediana de 30 rodadas por arquivo.
