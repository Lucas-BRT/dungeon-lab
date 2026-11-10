# Roteiro

O Tapiti é jovem. Esta página diz com franqueza o que está inacabado, para você saber no que pode confiar.

## Layout por crescimento (protótipo)

Além do layout padrão (uma sala por bloco da grade), o gerador pode *crescer* salas uma a partir da outra, na porta de uma sala-pai, em espaço livre. Isso dá um andar orgânico, fora da malha, e escala bem em mundos grandes (cerca de 165 ms para 81 salas numa grade 16×12), mas continua marcado como protótipo porque:

- **Muitas vezes para antes do número de salas pedido.** Com os parâmetros padrão chega a cerca de 75 a 80% das salas que o layout em grade daria.
- **Andares demais são rejeitados porque a escada fica perto da entrada** (5 a 10% falham depois de todas as tentativas nos níveis 1 e 5, e um andar custa de duas a quatro tentativas em vez de uma). O crescimento deveria favorecer profundidade, e a interface deveria checar `depth` contra `minHops`.
- **Os perfis de topologia quase não combinam com ele** (`backbone`, `redundantExit` e `longLoop` raramente validam); `hub` funciona.
- **Câmaras-folha não são geradas**, embora seus parâmetros continuem visíveis.
- **Ainda não tem testes automatizados**, e o antigo experimento `frontier-growth` continua no gerador atrás de uma feature.
- **O laboratório no navegador não tem controles para ele.**

## Outros itens em aberto

- A **condutância das junções** (`junction`) é um parâmetro do modelo de referência que o gerador ainda não usa.
- A **proximidade de Delaunay** está no modelo, mas não no gerador (RNG, Gabriel e Perto estão).
- A **pontuação global dos perfis de topologia**, os **objetivos de missão** e as **campanhas com vários pisos** estão descritos no [modelo](model) como propostas.
- **O laboratório como o app desktop.** O laboratório no navegador é a página original. Compilar a interface do desktop (egui) para WebAssembly unificaria os dois, e está adiado.
- **Publicar os crates** no crates.io e releases com binários do desktop.

## Pronto

- Gerador Rust, validador (V1 a V5) e medidas do grafo, determinístico byte a byte.
- App desktop nativo com desenho progressivo, em inglês e português.
- Este site, em inglês e português.

Achou algo faltando? [Abra uma issue](https://github.com/Lucas-BRT/tapiti/issues).
