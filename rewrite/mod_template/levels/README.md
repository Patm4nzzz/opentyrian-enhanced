`epN_NN.json` files written by the editor ("Export to the mod") or by hand:

```json
{ "episode": 1, "lvl_file_num": 9,
  "add_events": [ { "time": 300, "spawn": "my_drone", "x": 130 } ] }
```

`events` replaces the original list, `add_events` appends to it. Each event has a `time` and a
type key (`spawn`, `stop_map`, `jump`…). Types and parameters: MODDING.md §8.
