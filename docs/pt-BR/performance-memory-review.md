# Benchmarks

Como medir o gerador nativamente.

O benchmark nativo mede separadamente a geração (`generate`), geração com validação e tentativas (`stage_with_validation`), validação isolada e cálculo de métricas. Inclui os níveis 1–5, um cenário com grade irregular, formas giradas e rotas W, e um cenário de espinha dorsal com chaves, interruptor e ritmo ativados.

```sh
cargo bench --bench generator
```

Para guardar e comparar uma referência local:

```sh
cargo bench --bench generator -- --save-baseline antes
# depois de uma otimização:
cargo bench --bench generator -- --baseline antes
```

Criterion guarda os dados e relatórios sob `target/criterion`. O benchmark mede código Rust nativo; ele não mede serialização WebAssembly, transferência JSON nem desenho no navegador. Para memória heap e contagem de alocações, o próximo passo é acrescentar um profiler como DHAT ou heaptrack e comparar com estes mesmos cenários.

Com a biblioteca publicada como `cdylib` e `rlib`, o Cargo atual mostra um aviso de colisão de nomes de artefatos ao compilar o benchmark. A execução termina normalmente. Se uma versão futura do Cargo transformar esse aviso em erro, a saída do benchmark precisará ser movida para um pacote nativo separado.
