# Roteiro

O Tapiti é jovem. Esta página diz com franqueza o que está inacabado, para você saber no que pode confiar.

## Layout por crescimento (protótipo)

Além do layout padrão (uma sala por bloco da grade), o gerador pode *crescer* salas uma a partir da outra, na porta de uma sala-pai, em espaço livre. Isso dá um andar orgânico, fora da malha, e escala bem em mundos grandes (cerca de 165 ms para 81 salas numa grade 16×12), mas continua marcado como protótipo: muitas vezes fica abaixo do número de salas pedido, muitos andares são rejeitados porque a escada fica perto demais da entrada, a maioria dos perfis de topologia não se encaixa nele e as câmaras-folha não são geradas. Ainda não tem testes automatizados.

## Outros itens em aberto

- A **condutância das junções** (`junction`) é um parâmetro do modelo de referência que o gerador ainda não usa.
- A **proximidade de Delaunay** está no modelo, mas não no gerador (RNG, Gabriel e Perto estão).
- A **pontuação global dos perfis de topologia**, os **objetivos de missão** e as **campanhas com vários pisos** estão descritos no [modelo](model) como propostas.
- **O laboratório como o app desktop.** O laboratório no navegador é uma página Topcoat com seu próprio canvas, só em inglês. Compilar a interface do desktop (egui) para WebAssembly unificaria os dois, e está adiado.
- **Publicar os crates** no crates.io e releases com binários do desktop.

## Pronto

- Gerador Rust, validador (V1 a V5) e medidas do grafo, determinístico byte a byte.
- App desktop nativo com desenho progressivo, em inglês e português.
- Este site, em inglês e português.

Achou algo faltando? [Abra uma issue](https://github.com/Lucas-BRT/tapiti/issues).
