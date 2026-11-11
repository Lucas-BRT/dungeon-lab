# Primeiros passos

O Tapiti é um workspace Rust. Você precisa de uma toolchain estável recente ([rustup](https://rustup.rs)); nada mais para experimentar o gerador. O app desktop também precisa das bibliotecas gráficas usuais da sua plataforma (no Linux: xkbcommon, Wayland e/ou X11 e OpenGL).

```sh
git clone https://github.com/Lucas-BRT/tapiti
cd tapiti
```

## O app desktop

```sh
cargo run --release
```

Escolha um nível (1 a 5), mude a semente e edite qualquer parâmetro: o andar é cavado de novo um instante depois de cada mudança. O painel lateral tem três abas: a leitura do andar e o layout, todos os parâmetros de Λ e a análise. O idioma (inglês ou português) e o tema ficam na barra inferior.

Atalhos: <kbd>Alt</kbd>+<kbd>1</kbd> a <kbd>5</kbd> trocam o nível, <kbd>Alt</kbd>+<kbd>R</kbd> sorteia uma semente, <kbd>Alt</kbd>+<kbd>←</kbd> e <kbd>→</kbd> andam pelas sementes. Arraste o mapa para mover, use a roda para o zoom e dê um duplo clique para ajustar.

## O site e o laboratório

O site que você está lendo é um app [Topcoat](https://github.com/tokio-rs/topcoat). Para rodá-lo com recarga automática:

```sh
cargo install topcoat-cli
scripts/build-wasm.sh                      # o módulo WebAssembly do laboratório
topcoat dev -p tapiti-site      # http://127.0.0.1:3000
```

O laboratório (`/en/lab`) roda o gerador no seu navegador, compilado para WebAssembly. O formulário vem do mesmo esquema de parâmetros do app desktop. O laboratório é só em inglês.

## Docker

```sh
docker compose -f compose.local.yml up --build
```

serve o mesmo site em <http://localhost:8090>. O `compose.yml` é a versão pensada para o Dokploy; defina `TAPITI_SITE_URL` com o endereço público para que os links canônicos e as prévias sociais apontem para ele.

## Usando o crate

O `tapiti-core` é o gerador, sem interface. Adicione-o como dependência git:

```toml
[dependencies]
tapiti-core = { git = "https://github.com/Lucas-BRT/tapiti" }
```

```rust
use tapiti_core::{params::params_of_level, stage::stage};

fn main() {
    let params = params_of_level(5);
    // Até 8 tentativas: um andar que o validador rejeita é cavado de novo a partir de uma semente derivada.
    let staged = stage(42, &params, 8);
    assert!(staged.violations.is_empty());
    println!("{} salas, {} corredores", staged.d.rooms.len(), staged.d.edges.len());
}
```

A mesma semente e os mesmos parâmetros sempre dão o mesmo andar, nativo ou em WebAssembly.

## Ferramentas de linha de comando

```sh
cargo run --release -p tapiti-core --example dump               # um hash por estágio, para comparar dois builds
cargo run -p tapiti-render --example export_svg -- 5 26 floor.svg dark   # nível, semente, saída, tema
cargo bench -p tapiti-core --bench generator                    # benchmarks Criterion
```

## Para onde ir depois

- [O modelo matemático](model): o que é um andar, como é validado e medido.
- [Parâmetros](parameters): cada parâmetro de Λ, com sua faixa.
- [Benchmarks](performance): como medir o gerador.
- [Roteiro](roadmap): o que está inacabado.
