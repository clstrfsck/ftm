# PILOT

> **This document is still a draft, and it is not normative yet.** What follows
> is general advice on writing an automated falling-block player, gathered
> before any of it was written against this game. Stage P1 of `PILOT-PLAN.md`
> replaces it with the normative design and gives it the section numbers
> **§P1-§P9**, which is how the rest of the source will cite it. Until then
> nothing here binds the implementation, and nothing in `FTM.md`, `TUI.md` or
> `GUI.md` refers to it.
>
> The mode is called **PILOT**. It is not called ROBOTO: that is Google's
> typeface, and §1.3 keeps a trademarked word out of the game rather than
> beside it — the same decision that made a four-line clear a `QUAD`.

A good automated Tetris player should use **lookahead search with a heuristic evaluation function**. Rather than encoding fixed stacking rules, enumerate legal placements, simulate their results, score the resulting boards, and choose the best move.

## Recommended approach

For each piece:

1. Enumerate every reachable rotation and horizontal position.
2. Simulate dropping the piece there.
3. Clear completed lines.
4. Evaluate the resulting board.
5. Repeat recursively for the next 1–3 preview pieces.
6. Choose the first move leading to the highest total score.

A depth of 2–3 pieces is often enough for a capable real-time bot. Beam search can control the computational cost.

## Board evaluation

Use a weighted combination of features:

```text
score =
    + line_clear_reward
    - aggregate_height_penalty
    - hole_penalty
    - bumpiness_penalty
    - maximum_height_penalty
    - covered_hole_penalty
    - well_penalty
```

The most useful features are:

- **Holes:** Empty cells with occupied cells above them. Penalize these heavily.
- **Covered-hole depth:** Count how many blocks would need removal to expose each hole.
- **Aggregate height:** Sum of all column heights.
- **Maximum height:** Prevents risky towers and top-outs.
- **Bumpiness:** Sum of height differences between adjacent columns.
- **Completed lines:** Reward immediate line clears.
- **Wells:** Deep gaps between taller neighboring columns. Penalize accidental wells, but allow a deliberate Tetris well.
- **Row transitions:** Filled-to-empty changes across each row; fewer generally means a cleaner board.
- **Column transitions:** Filled-to-empty changes down columns.
- **Blockades:** Blocks positioned above holes.

A simple starting evaluator could be:

```text
score =
    0.8  × completed_lines
  - 0.5  × aggregate_height
  - 0.3  × bumpiness
  - 1.5  × holes
  - 0.8  × covered_hole_depth
  - 0.7  × maximum_height
```

These values are only starting points; their scale depends on how each feature is measured.

## Search structure

```python
def choose_move(board, pieces, depth):
    best_move = None
    best_value = float("-inf")

    for move in legal_placements(board, pieces[0]):
        next_board, lines = simulate(board, pieces[0], move)

        value = evaluate(next_board, lines)

        if depth > 1:
            value += discount * search(
                next_board,
                pieces[1:],
                depth - 1
            )

        if value > best_value:
            best_value = value
            best_move = move

    return best_move
```

For deeper searches, do not simply add a full evaluation at every depth, because that can count persistent board features repeatedly. A cleaner design is:

```text
value = immediate_clear_reward
      + discount × future_value
```

Then apply the full board evaluation at leaf nodes.

## Beam search

The number of possibilities grows rapidly—often dozens of placements per piece. Beam search makes deeper lookahead practical:

1. Generate all states at the next depth.
2. Deduplicate identical boards.
3. Evaluate them approximately.
4. Keep only the best *k* states, such as 50–500.
5. Expand those states for the next piece.

This is usually more useful than exhaustive search at the same computational budget.

## Unknown future pieces

How to handle uncertainty depends on the rules:

- **Visible queue:** Search exactly over the known pieces.
- **7-bag randomizer:** Track which pieces remain in the current bag.
- **Independent random pieces:** Use expectimax over possible next pieces.
- **Performance-critical bot:** Sample several plausible future sequences rather than expanding every possibility.

With uncertainty, select the move with the best expected result:

```text
move value = average outcome across likely future piece sequences
```

For survival-focused play, consider penalizing bad worst-case outcomes as well:

```text
robust value = 0.8 × expected value + 0.2 × worst-case value
```

This prevents the bot from choosing fragile setups that work only if a particular piece arrives.

## Reachability matters

Do not consider every geometrically valid final placement automatically reachable. Model:

- Left and right movement
- Both rotation directions
- Gravity
- Lock delay
- Wall and floor kicks
- Hold
- Spawn collision
- Game-specific rotation rules

Use breadth-first search over states such as:

```text
(x, y, rotation, lock-delay state)
```

This yields valid input sequences and prevents the planner from selecting impossible placements. If the environment permits hard-drop placement from above without obstructions, a simpler placement generator may be sufficient initially.

## Hold handling

Treat Hold as an additional branch:

- Place the current piece.
- Swap with the held piece and place that instead.
- If Hold is empty, consume the next queue piece according to the game’s rules.

Remember that Hold can generally be used only once before locking a piece.

## Optimizing the weights

Hand-tuned weights work, but automated optimization is better. Define an objective such as:

- Average lines survived
- Sprint completion time
- Score over a fixed number of pieces
- Top-out rate
- Average score across many random seeds

Then optimize the weights using:

- Genetic algorithms
- Cross-entropy method
- CMA-ES
- Bayesian optimization
- Reinforcement learning

Evaluate every candidate over many identical seeds so random piece sequences do not dominate the comparison.

## Practical development path

1. Build a correct board simulator.
2. Enumerate legal terminal placements.
3. Implement one-piece heuristic search.
4. Add two- or three-piece lookahead.
5. Deduplicate equivalent boards.
6. Add beam search and caching.
7. Model Hold and the preview queue.
8. Tune weights automatically.
9. Add T-spin and combo features only if the scoring system rewards them.
10. Add reachable-movement planning for actual control inputs.

For a general-purpose bot, the best starting design is **two-piece lookahead, beam search, strong penalties for holes and height, board deduplication, and automatic weight tuning**. This is much easier to implement and debug than deep reinforcement learning, while still producing strong play.
