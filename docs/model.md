# Mathematical model of level generation

This document organizes the mathematics of the Tapiti generator around one simple question: **how do we build a map that is spatially valid, connected and interesting to explore?**

The answer has three parts. First we define rooms and corridors in the plane. Then we represent the connections as a graph and measure its topology. Finally we choose which properties must be mandatory and which are only preferences.

This is the mathematical specification behind Tapiti's generator. Since the Rust implementation does not cover every idea of the model, the text distinguishes the **reference model**, the **current behavior of the crate** and the **proposed extensions**.

## 1. The problem in one sentence

A level is a network of rooms and passages, placed in space, that must respect geometric constraints and offer an exploration experience with a desired structure.

We use the following representation:

$$
\mathcal D = (H, \varphi, \alpha, \Lambda)
$$

- $H=(V,E)$ is the **navigable graph**: vertices are rooms or junctions; edges are corridor stretches between those points.
- $\varphi$ is the **geometric realization**: it says where the rooms are and where the corridors run.
- $\alpha$ assigns roles to parts of the level, such as start, stairs, prize or ambush.
- $\Lambda$ gathers the generator's parameters, such as dimensions, clearances, shape weights and density.

The current generator builds a connected 2D floor. The representation separates topology — what connects to what — from geometry — how those connections fit in the plane. This separation allows specifying the shape of a network without requiring it to already have coordinates.

### Vocabulary and status

In this text, **reference model** identifies concepts preserved from the original mathematical specification; **current crate** indicates what exists in this Rust implementation; **proposal** identifies an extension not yet implemented. Topology profiles, keys, switches and exploration pacing are now part of the crate; multi-floor campaigns and mission objectives remain proposals.

## 2. Basic notation

| Symbol | Meaning |
|---|---|
| $\mathbb R^2$ | the floor plane, in pixels |
| $S_i\subset\mathbb R^2$ | region occupied by room $i$ |
| $\partial S_i$ | outline of room $i$ |
| $\operatorname{dist}(A,B)$ | smallest Euclidean distance between two sets |
| $w$ | corridor width |
| $\sigma$ | minimum clearance between regions that do not connect |
| $\ell_s$ | minimum length of the straight stretch right after a door |
| $\ell_{seg}$ | minimum length of a corridor segment |
| $\ell_{min}$ | desired minimum distance between connected rooms |
| $\lfloor x\rfloor$ | greatest integer less than or equal to $x$ |

A clearance is a distance between **region borders**, not between their centers. For example, requiring $\operatorname{dist}(S_i,S_j)\geq 2g$ means there are at least $2g$ pixels of space between the outlines of the two rooms.

## 3. Room geometry

### 3.1 Direction alphabet

Doors and corridor segments use a discrete set of directions:

$$
\Theta_m=\left\{\frac{2\pi k}{m}: k=0,\ldots,m-1\right\},
\qquad m\in\{4,8\}.
$$

With $m=4$, the directions are horizontal and vertical. With $m=8$, the 45° diagonals are added. We call the first case **rectilinear** and the second **octilinear**.

A free curve could join any two points, but discrete directions make passages more predictable and make it easier to test their width and intersections. This is a deliberate simplification of the model.

### 3.2 Shape classes

Each room occupies a region $S_i$ of a class $\kappa_i$:

$$
\mathcal S=\{\textsf{rect},\textsf{disc},\textsf{oct},\textsf{L},\textsf{star}\}.
$$

| Class | Main parameters | Area or construction |
|---|---|---|
| Rectangle | width $W$, height $H$ | $WH$ |
| Disc | radius $\rho$ | $\pi\rho^2$ |
| Octagon | circumradius $\rho$ | $4\rho^2\sin(\pi/4)$ |
| L | box $W\times H$, notch $a\times b$, corner $c$ | $WH-ab$ |
| Truncated star | number of points $n$, radii and top width $\tau$ | computed by the polygon (*shoelace*) formula |

A star has its points cut off. Without the cut, each point would be just a vertex and there would be no suitable stretch of edge to open a straight door.

In the reference model, a room is fitted into a usable rectangle $\beta$ by a function

$$
\operatorname{fit}_{\kappa}(\beta,\xi)\to S\subseteq\beta,
$$

where $\xi$ represents random choices such as proportions and orientation. In the crate, the blocks of the [scaffold](../crates/tapiti-core/src/generate.rs) also limit where rooms can be placed.

To keep doors aligned with the alphabet, the reference specification restricts the rotations of shapes to $\varphi\in\Theta_m$. This is a modeling choice: continuous rotations would require also accepting walls whose normals do not belong to the corridor alphabet.

## 4. Doors and room capacity

A door is a pair $p=(x,\theta)$: the point $x$ on the room's outline and the direction $\theta$ in which the corridor leaves.

On a polygon edge, not every point is admissible. The door must stay away from the corners and its exit must not cross the room itself. We define the inset

$$
\iota=\frac{w}{2}+\sigma.
$$

For an edge of length $L$, the admissible points initially lie in the interval $[\iota,L-\iota]$. In the mathematical model, any point of that interval can be chosen; the implementation samples positions in `portStep` steps and also adjusts doors when it resolves some intersections.

A door's exit strip can be represented as

$$
F(x,\theta)=\{x+s\,u(\theta)+r\,u(\theta+\tfrac\pi2):
0\leq s\leq\ell_s,\ |r|\leq w/2\},
$$

where $u(\theta)=(\cos\theta,\sin\theta)$. Intuitively, $F$ is the rectangle the corridor occupies right after leaving the wall.

Two doors used in the same room must keep their strips apart:

$$
\operatorname{dist}(F(p_i),F(p_j))\geq\sigma\quad(i\ne j).
$$

We call **capacity** of the room the largest number of doors that satisfies this condition:

$$
\operatorname{cap}(S)=\max\{|U|: U\text{ is a set of admissible, separated doors}\}.
$$

For an edge with an admissible interval of length $\lambda$, the limit is $\lfloor\lambda/(w+\sigma)\rfloor+1$. On a disc of radius $\rho$ with equally spaced doors, the distance between neighboring positions is the chord $2\rho\sin(\pi/q)$; all $q$ doors fit if that chord is at least $w+\sigma$. This computation shows why the radius limits the number of doors, even when the parameter asks for more directions.

For example, on the same straight side, two doors cannot be closer than roughly $w+\sigma$. Capacity therefore depends both on the room's size and on the corridors' width.

## 5. Corridors

### 5.1 A corridor as an occupied region

A corridor is made of connected segments. Each segment follows a direction in $\Theta_m$ and has a length of at least $\ell_{seg}$. The segment leaving a door must also keep the door's direction for at least $\ell_s$.

If $\gamma_e$ is the center line of a segment $e$, its occupied strip is the neighborhood of that line with radius $w/2$. A corridor's region is the union of the strips of its segments and of the caps at bends and junctions. Collision checks therefore compare regions, not just center lines.

### 5.2 Kinds of link

Between two doors, the specification considers three kinds of route:

| Type | Bends | Description |
|---|---:|---|
| I | 0 | straight segment between aligned doors facing each other |
| L | 1 | two rays that meet in front of the doors |
| Z/U | 2 | two exit stretches joined by an intermediate segment |

For example, for a straight route between $(p_a,\theta_a)$ and $(p_b,\theta_b)$, the doors must face each other, $p_b-p_a$ must be aligned with the exit direction, and the distance must allow the two minimum stretches:

$$
\theta_b\equiv\theta_a+\pi\pmod{2\pi},\qquad
(p_b-p_a)\times u(\theta_a)=0,\qquad
(p_b-p_a)\cdot u(\theta_a)\geq 2\ell_s.
$$

L and Z routes impose similar conditions of alignment, direction and minimum length for each segment. In the [Rust generator](../crates/tapiti-core/src/generate.rs), direct routes follow the same I/L/Z family, with the bend limit controlled by `maxBends`.

For an L route, let $u_a=u(\theta_a)$, $u_b=u(\theta_b)$, $d=p_b-p_a$. If $u_a\times u_b\neq0$, the rays meet at the parameters

$$
t=\frac{d\times u_b}{u_a\times u_b},\qquad
s=\frac{d\times u_a}{u_a\times u_b}.
$$

The route is feasible only if $t,s\geq\ell_s$ and the bend formed by the directions is allowed. For Z, we choose an intermediate direction $u(\delta)$ and solve

$$
p_b-p_a=t_a u_a+t_c u(\delta)-t_bu_b,
\qquad t_a,t_b\geq\ell_s,\quad t_c\geq\ell_{seg}.
$$

The minus sign appears because $u_b$ points out of the second room; the route's last segment reaches the door in the opposite direction. There are two vector equations in three lengths: for each intermediate direction there is a family of solutions. The specification fixes one length or a ratio between stretches and solves for the others.

### 5.3 Junctions

A junction connects three arms (T) or four arms (X). The arms share a meeting point, which is also a vertex of the navigable graph. This representation matters for counting paths and cycles: a T junction is a bifurcation, not a triangle of passages.

The reference specification also describes a **conductance** $\omega_h$ between the arms of a junction. It can model whether two arms behave as a single space or as partly coupled regions. The `junction` field appears in the parameters, but is marked in the code as unused by the generator; this interpretation is therefore an extension of the space model, not an effect the current crate produces.

### 5.4 Cost and weight of a route

A route can be geometrically valid and still be less desirable than another. To rank candidates, we use an aesthetic cost:

$$
C(k)=c_b b(k)+c_\ell\ell(k)+c_{45}n_{45}(k)
-c_f\,\mathbf 1_{\text{facing doors}},
$$

where $b(k)$ is the number of bends, $\ell(k)$ the length and $n_{45}(k)$ the number of 45° bends. The indicator $\mathbf 1$ is 1 when the condition is true and 0 when it is not.

The cost does not decide validity. Among valid candidates, the weight is

$$
\omega(k)=M_{\kappa_a\kappa_b}\,p_{type(k)}\,e^{-C(k)},
$$

where $M$ is the compatibility between the room classes and $p_{type(k)}$ is the weight of the route type. A zero weight forbids the link; a larger weight raises its chance in a weighted choice. This is the mechanism used to vary choices without abandoning the geometric constraints.

## 6. Navigable graph

### 6.1 The representation used for topology

The refined graph of the level is

$$
H=(V,E),\qquad V=R\cup J,
$$

where $R$ is the set of rooms, $J$ the set of junctions, and each edge in $E$ is a corridor arm between a room and a junction, two junctions or two rooms. The graph may have parallel edges: two physically distinct passages between the same vertices remain two edges.

We use $H$ as the main representation for connectivity, cycles and independent routes. A projection that links every room of a junction to all the other rooms of that junction may be useful for certain "hops between rooms" measures, but it creates artificial connections if used to count cycles.

**Example.** Three rooms joined by a single T junction form a tree in $H$: three arms and three rooms, no cycle. If we project the T as links between every pair of rooms, a triangle appears. That triangle does not correspond to three independent passages; all of them use the same junction. This is why the clique expansion must not be used to compute $\beta$ or edge-disjoint routes.

### 6.2 Connectivity

A connected level has a path between any pair of rooms. If $c(H)$ is the number of connected components, the general condition is $c(H)=1$ for a floor that contains a single playable region.

A **bridge** is an edge whose removal disconnects the graph. An **articulation vertex** is a vertex whose removal disconnects it. These two structures identify bottlenecks: places exploration has to pass through.

### 6.3 Number of cycles

The number of independent cycles, also called the **cyclomatic number**, is

$$
\beta(H)=|E|-|V|+c(H).
$$

For a connected floor, $\beta=|E|-|V|+1$. A tree has $\beta=0$. Adding a link between two already connected points raises $\beta$ by 1.

**Example.** The path room A — room B — room C has 3 vertices and 2 edges, so $\beta=2-3+1=0$. Adding a passage between A and C gives 3 edges and 3 vertices; then $\beta=3-3+1=1$, corresponding to one independent cycle.

If $H$ is planar and connected, Euler's formula gives $|V|-|E|+|F|=2$. Hence $\beta=|F|-1$: it is the number of bounded faces of the drawing. This interpretation depends on planarity; the cyclomatic number remains defined even when the face reading is not used.

### 6.4 Cycles do not guarantee alternative routes to the exit

The number of cycles describes the whole network, but not where the cycles are. To measure independent routes between start $s$ and stairs $t$, we use

$$
\lambda_H(s,t)=\text{maximum number of paths from }s\text{ to }t
\text{ that share no edges}.
$$

By Menger's theorem, this number is also the minimum size of a set of edges whose removal separates $s$ from $t$.

So $\beta(H)>0$ does **not** imply $\lambda_H(s,t)>1$. For example, consider the path $s-a-t$ and a wing in a cycle $a-b-c-a$. The graph has 5 vertices and 5 edges, so $\beta=5-5+1=1$, but every route from $s$ to $t$ still uses the edge $a-t$: $\lambda_H(s,t)=1$. By contrast, two routes $s-a-t$ and $s-b-t$ have $\beta=1$ and $\lambda_H(s,t)=2$. The level profile must specify "how many cycles exist" and "how many routes reach the exit" separately.

Other useful measures include:

| Measure | Definition | What it describes |
|---|---|---|
| Room distance $d_{room}(s,t)$ | shortest path counting each arrival at a room as 1 and passing through a junction as 0 | length of the main route in rooms crossed |
| Geometric distance $d_{geom}(s,t)$ | smallest sum of corridor lengths between start and exit | minimum physical walk |
| Diameter | largest minimum distance between pairs of rooms | perceived global size |
| Girth $g(H)$ | size of the smallest cycle (defined as $+\infty$ if there is no cycle) | presence of short cycles |
| Degree $d(v)$ | number of edges incident to $v$ | local number of options |
| Articulations and bridges | vertices/edges whose removal disconnects $H$ | structural bottlenecks |
| $\lambda_H(s,t)$ | edge-disjoint routes between start and exit | redundancy of the final route |
| $\lambda_2$ | second smallest eigenvalue of the Laplacian $L=D-A$ | global connectivity, in aggregate |

### 6.5 When a level is valid

The constraints below separate different problems. **Connectivity** is about the network; **clearance** is about geometry; **accessibility** is about the game rules.

- **V1 — Connectivity.** All relevant navigable vertices belong to the same component of $H$.
- **V2 — Clearances.** Rooms keep the minimum distance $2g$; a corridor stays at least $\sigma$ from the rooms it does not connect to; two separate corridors keep the clearance, except at a valid junction. Between linked rooms, the direct distance $\ell_{min}$ is also required, so that rooms close to each other diagonally do not become nearly adjacent despite a long path.
- **V3 — Doors.** Each used door is admissible for its room, and the exit strips of the doors of the same room are separated by at least $\sigma$.
- **V4 — Corridors.** Directions, bends and lengths respect $\Theta_m$, $\ell_s$, $\ell_{seg}$ and the bend limit.
- **V5 — Start and exit.** There is a start room and a distinct stairs room; the stairs respect the door limit and are at least `minHops` room hops from the start.
- **V6 — Furniture.** In the original specification, stones and crystals must also stay out of the exit strips and keep their distance from each other. This step belongs to the game's furniture and is not implemented by this crate's geometric generator.

A level must satisfy all the constraints that apply to its scope. A lower aesthetic cost cannot compensate for a violation of V1–V5.

## 7. Cycle density

To normalize the number of cycles, the specification defines a reference $\beta_{ref}$ and a density:

$$
\delta_{target}=
\begin{cases}
\beta^*/\beta_{ref}, & \beta_{ref}>0,\\
0, & \beta_{ref}=0.
\end{cases}
\qquad 0\leq\delta_{target}\leq1.
$$

In the interface and in the code, this reference may appear under the name `betaMax`. The name must be read carefully: the value is obtained by a greedy strategy that adds the highest-weight valid actions until no options remain. This produces a **maximal reference of the heuristic**, not a proof of the global maximum among all possible configurations.

The target is rounded from $\delta_{target}\beta_{ref}$. For example, if $\beta_{ref}=8$ and $\delta_{target}=0.25$, the target will be about 2 cycles. The realized density is $\delta_{real}=\beta(H)/\beta_{ref}$ when $\beta_{ref}>0$; it may differ from the target because of rounding, because an X action adds two cycles, or because $\beta_{ref}$ comes from a heuristic, not a proven maximum. In some cases, $\delta_{real}$ can even exceed 1. The target controls the approximate quantity; topology profiles are needed to control the location and function of those cycles.

For a connected graph, the average degree satisfies

$$
\langle d\rangle=\frac{2|E|}{|V|}
=\frac{2(|V|-1+\beta)}{|V|}.
$$

This identity relates the number of cycles to the average degree, but does not determine the layout of the network.

## 8. Generation in the reference model

The process can be understood as six steps. The original specification describes this sequence; the Rust crate implements a variant, summarized in section 12.

1. **Scaffold.** Create a grid of $C\times R_g$ cells and combine them into blocks. Some cells may be dropped, as long as the remaining blocks stay connected.
2. **Rooms.** Draw a shape class for each block and fit the region into the usable space, respecting limits and clearances.
3. **Candidates.** Choose pairs of rooms to consider; for each pair, look for feasible doors and routes.
4. **Conflicts.** Mark pairs of candidates that cannot coexist because of geometric collision or doors that are too close.
5. **Tree and cycles.** First connect the rooms with a spanning tree. Then add links between already connected points until approaching the $\beta$ target.
6. **Leaf rooms and roles.** Use dropped cells for degree-1 rooms and assign roles such as start, stairs, prize or trap.

A tree guarantees connectivity without cycles. The second phase adds alternatives without removing existing connections.

### 8.1 Mixing room classes

A class distribution can consider both the block size and the classes of the neighboring blocks. Let $N(i)$ be the set of blocks adjacent to $i$, $n_i$ the number of cells of $i$, and $n_\kappa(i)$ how many neighbors have class $\kappa$. The sampling rule is

$$
P(\kappa_i=\kappa\mid\kappa_{N(i)})
\propto w_\kappa(n_i)\exp\bigl(h\,n_\kappa(i)\bigr).
$$

The weight $w_\kappa(n_i)$ can, for example, forbid stars in small blocks. The parameter $h$ controls homophily: $h>0$ favors neighbors of the same class; $h<0$ favors different classes. In the crate, this rule is sampled in four sweeps and then the `maxOf` quotas are applied.

### 8.2 Candidate room pairs

The reference specification considers proximity graphs built from the room centers:

- **RNG**: a link between $i$ and $j$ is removed if another room is closer to both;
- **Gabriel**: a link is removed if another room falls inside the disc whose diameter is the segment between the centers of $i$ and $j$;
- **Bounded Delaunay**: consider Delaunay neighbors up to a maximum distance.

If $c_i$ is the center of room $i$ and $d_{ij}=\|c_i-c_j\|^2$, the RNG test keeps $\{i,j\}$ when there is no $k$ with $\max(d_{ik},d_{jk})<d_{ij}$. The Gabriel test keeps the link when there is no $k$ with $\|c_k-(c_i+c_j)/2\|^2<d_{ij}/4$. The `Near` variant used in the crate keeps pairs with $\|c_i-c_j\|\leq r$.

These filters control which pairs enter the search. They do not by themselves guarantee that the rooms can be connected: doors and corridors without collisions must still be found.

### 8.3 Conflict graph

Let $\mathcal C$ be the set of candidate links. We build a conflict graph $X=(\mathcal C,F_X)$, in which two candidates are joined in $F_X$ when they cannot be chosen together.

A conflict-free selection is an **independent set** of $X$. To form a connected level, we also require the selection to build a connected navigable graph. This formulation separates two questions:

1. is each link valid on its own?
2. can the chosen links coexist?

### 8.4 Tree, cycles and leaf rooms

The crate chooses a start room and grows a spanning tree. At each step, a candidate link must connect an already reached room to a new room. The choice is weighted by the candidates' weights. If the current list does not allow progress, the generator widens the set of proximity pairs and tries to continue.

After connecting the rooms, the generator builds a `betaMax` reference by filling a copy of the graph with the highest-weight valid actions. This search considers direct links and T/X junction actions. The reference is greedy: it depends on earlier choices and does not prove the global maximum.

In the real cycles phase, the target is $\beta^*=\operatorname{round}(\delta_{target}\beta_{ref})$, where `delta` is the crate's parameter. Valid actions are drawn until the target is reached or exceeded, or until no actions remain. A direct or T link adds one independent cycle; an X crossing can add two, since it closes a new path and splits the crossed corridor into two branches. So $\beta^*$ is a target, not a guarantee of exact equality.

Leaf chambers are added afterwards: each is a room linked by a single corridor. In the graph, the operation adds one vertex and one edge, so $\Delta\beta=0$. It raises the number of dead ends without raising the number of cycles. The roles of start, stairs, boss, rest, ambush, trap, prize and empty room describe content. Keys, switches and locks are progression data, separate from the labels.

In the crate, the start is chosen among the live rooms. After the tree, the stairs are chosen among the leaves farthest from the start; if the minimum distance `minHops` is not reached, `stage` may try another seed up to the attempt limit.

## 9. Topology profiles

The `topology` parameter lets you ask explicitly for a **topological intent**. The crate implements mandatory requirements for branches, depth, independent routes to the exit, hub degree and minimum cycle length. If the geometry does not allow a requirement to be met, the generator records the limitation and the validator points out the violation.

We represent a profile as

$$
\Pi=(C_{hard},\mathbf t,\mathbf w),
$$

where:

- $C_{hard}$ is a set of mandatory conditions;
- $\mathbf t$ is the vector of target values for the graph measures;
- $\mathbf w$ defines the relative importance of each target.

Let $\Phi(H)$ be the vector of metrics, for example

$$
\Phi(H)=
(\beta,\ g(H),\ \lambda_H(s,t),\ d_{room}(s,t),\ d_{geom}(s,t),\
\text{bridges},\text{articulations},\text{degrees},\text{branches}).
$$

The feasible set contains only levels that satisfy geometric validity and the coded mandatory conditions. The crate does not yet search for the best level according to a global scoring function. The expression below formalizes a possible extension to compare metrics with targets:

$$
J_{\Pi}(H)=\sum_i w_i\,\rho_i(\Phi_i(H),t_i),
$$

where $\rho_i$ penalizes the distance from the target. The mandatory constraints filter the levels; the function $J_\Pi$ would rank the remaining options. This global optimization remains outside the current crate.

### 9.1 Example profiles

**Spine with optional wings.** We choose a main path $P$ from $s$ to $t$. We remove the vertices of $P$ from $H$ and examine each remaining component $C$. We define its neighborhood on the spine by

$$
\partial_P C=\{e\in E:e\text{ has one end in }C\text{ and the other in }V(P)\}.
$$

If $|\partial_P C|=1$, $C$ is a simple side branch: a single edge links the component to the spine. If $|\partial_P C|\geq2$, $C$ offers a detour that returns to the spine — a shortcut or cycle, not a simple branch. We count edges, not just contact vertices: two parallel passages to the same vertex of $P$ already form a second link and must not be classified as a simple branch.

**Two routes to the exit.** Require $\lambda_H(s,t)\geq2$. This guarantees that no single edge is enough to separate start and exit.

**Explorable loop.** Require a simple cycle $C\subseteq H$ with at least $L_{loop}$ edges. The condition $\beta\geq1$ alone would allow the only cycle to be very small.

**Hub.** Require a room or junction $h$ with degree $d(h)\geq d_{hub}$, or favor a degree distribution that concentrates several options in one region.

**Intentional bottleneck.** Require a bridge or articulation at a specified stretch of the main path. This can concentrate the level's pacing, as long as progression allows crossing it.

These requirements can coexist. For example, a profile can require a long spine, two branches with rewards and two alternative routes in the final stretch.

## 10. Progression with keys and switches

Topology answers "which places connect?". Progression answers "when can the player cross each connection?". To model locked doors and objectives, we extend the player's state:

$$
q=(I,S,O),
$$

where $I$ is the inventory, $S$ the set of activated mechanisms and $O$ the completed objectives.

An edge can be crossed only if its condition is satisfied in $q$. The crate implements masks of up to eight keys and eight switches and checks the state space of location and inventory. A key is collected on entering the room; a switch is activated on entering it.

To keep each lock meaningful even when there are shortcuts, the generator splits the graph by successive cuts of the main path and applies the condition to all the edges that cross each cut. So it is not enough to place a key before a chosen door: every path that goes past that point must satisfy the condition. The validator walks states $(v,I,S)$ and requires the stairs to remain reachable.

The `pacing.enabled` field distributes rest, boss and ambush rooms along the main path and raises the preference for prizes in optional branches. These roles describe pacing and content; they do not create mission objectives.

The same model can extend to a multi-floor campaign: we assign each room a floor index and let stair or elevator edges connect vertices on different floors. Each floor has its own geometric realization $\varphi_f$, but connectivity and progression are checked on the joint graph. This extension is not yet implemented in this crate.

## 11. Model parameters

The parameters can be grouped by function. The names in parentheses are those used in the crate when there is a direct match.

| Group | Parameters | Effect |
|---|---|---|
| Scaffold | columns, rows, cell size, inset, large blocks, dropped fraction, irregularity and rotation (`cols`, `rows`, `cellW`, `cellH`, `gap`, `big`, `drop`, `layoutJitter`, `rotateRooms`) | scale, fragmentation and orientation of rooms; irregularity is limited to preserve the minimum size |
| Shapes | weights and quotas per class, minimum size for a star, homophily (`weights`, `maxOf`, `starCells`, `homophily`) | variety and distribution of rooms |
| Doors | allowed directions, number of doors on discs, sampling step (`m`, `discPorts`, `portStep`) | positions and directions of the connections |
| Corridors | width, clearance, exit stretch, minimum segment, minimum distance, maximum bends (`width`, `clear`, `exit`, `seg`, `minLen`, `maxBends`) | geometry of the passages |
| Weights | probabilities per route type, including the W detour, bend/length/diagonal costs (`pI`, `pL`, `pZ`, `pW`, `pT`, `pX`, `costBend`, `costLen`, `cost45`, `costFacing`) | preference among valid candidates |
| Topology | proximity, radius, density and profile (`proximity`, `near`, `delta`, `topology`) | pairs considered, cycle target and network requirements |
| Progression | number of key and switch locks (`progression`) | crossing conditions checked in the state graph |
| Pacing | roles distributed along the main path (`pacing`) | contrast between advancing, resting, combat and optional exploration |
| Start and exit | stairs door limit and minimum distance in hops (`stairsDoors`, `minHops`) | topological position of the stairs |
| Content | number of ambushes and leaf chambers; allowed classes and roles (`ambush`, `leaves`, `leafClasses`, `leafRoles`) | distribution of existing functions |
| Mixing | compatibility matrix between classes (`mix`) | allows or discourages connections between shapes |
| Junctions | optional conductance (`junction`) | reference parameter, not yet used by the generator |

The profile $\Pi$ complements $\Lambda$: the parameters control how the generator operates; the profile describes the structure you want to obtain.

## 12. What exists in the Rust crate

This table avoids confusing the reference specification with this repository's implementation.

| Concept | Status in the crate |
|---|---|
| Rooms and geometries | Rectangles, discs, octagons, L shapes and stars; placement based on a grid of blocks. |
| Directions and routes | Modes with 4 or 8 directions; I/L/Z candidates, T branches and X crossings. |
| Main connection | Tree growth over candidate room pairs; the generator tries other proximity relations if it runs out of connections. |
| Cycles | Later choice of cycle actions up to the target derived from `delta` and the `betaMax` reference. |
| Start and stairs | One start and one stairs room per floor; the stairs are chosen among distant rooms, usually leaves. |
| Room content | Marking of start, stairs, boss, rest, ambush, trap, prize, key, switch or empty. The labels are not a mission system. |
| Proximity | The Rust enum includes RNG, Gabriel and radius-based Near. Delaunay appears in the reference specification, but not in the current enum. |
| Topology profiles | Requirements for branches, depth, independent routes, hub and minimum cycle implemented; global profile scoring not yet implemented. |
| Stateful progression | Keys, switches, conditional doors and reachability checking implemented; mission objectives not yet implemented. |
| Exploration pacing | Rest, boss and ambush roles on the main path and a higher preference for prizes in optional branches. |
| Spatial flexibility | Irregular grid limited by minimum dimensions, shape rotation and W routes of up to six bends. |
| Connected multiple floors | Each current generation produces a single 2D map; the progression relationship between floors is not modeled here. |

In the crate, the geometric and topological checks are spread mainly across [generate.rs](../crates/tapiti-core/src/generate.rs), [validate.rs](../crates/tapiti-core/src/validate.rs) and [metrics.rs](../crates/tapiti-core/src/metrics.rs). The validator is the reference for deciding whether a concrete output satisfies the coded conditions.

## 13. Summary

The model separates three questions:

1. **Does the geometry fit?** Do rooms, doors and corridors respect widths and clearances?
2. **Does the network have the desired shape?** Do the cycles, branches, bottlenecks and routes to the exit match the profile?
3. **Can the level be completed?** If there are progression rules, does the exit remain reachable in the right state?

This separation keeps the mathematics clear. Geometry defines what is physically possible; the graph defines the navigation structure; profiles define the level's intent. The next natural step is to express intent as conditions on $H$, without confusing the global number of cycles with the availability of alternative routes at specific points.
