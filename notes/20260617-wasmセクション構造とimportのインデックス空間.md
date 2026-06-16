# wasm セクション構造と import の関数インデックス空間

「import はインデックス空間の先頭を占める」という主張の根拠を、wasm のバイナリ構造に即して示す。
実機データはすべて `sandbox-wasm/target/wasm32-unknown-unknown/release/wasm_experiment.wasm`
（cxx + libcxx 実験ビルド）を `llvm-objdump` / node の `WebAssembly.Module.imports` で読んだもの。

## 1. モジュール全体の構造

wasm ファイルは「8 バイトの先頭ヘッダ + セクションの並び」でできている。

```
先頭バイト（実測）: 00 61 73 6d 01 00 00 00
                    └── magic ──┘ └─ version=1 ─┘
                    "\0asm"
```

その後ろに、各セクションが `[id:1byte][size:LEB128][payload]` の形で**並ぶ**。
既知セクションは id 昇順で現れる決まり（custom=id0 は任意位置）。実測のセクション一覧:

| 並び | セクション | 実 id | サイズ(実測) | 役割 |
|---|---|---|---|---|
| 0 | TYPE | 1 | 0x0af | 関数シグネチャ（型）の表 |
| 1 | **IMPORT** | **2** | 0x0b2 | **外部から取り込む要素の宣言**（関数・table・memory・global） |
| 2 | FUNCTION | 3 | 0x1aa | 自前定義関数の「**型インデックスだけ**」の表（本体は無い） |
| 3 | TABLE | 4 | 0x005 | 関数テーブル（間接呼び出し用） |
| 4 | MEMORY | 5 | 0x003 | リニアメモリ |
| 5 | GLOBAL | 6 | 0x019 | グローバル変数 |
| 6 | EXPORT | 7 | 0x226a | 外部公開する要素 |
| 7 | ELEM | 9 | 0x02e | テーブル初期化（どの funcidx を載せるか＝アドレス取得） |
| 8 | CODE | 10 | 0xd4af | **自前定義関数の本体（命令列）** |
| 9 | DATA | 11 | 0x138f | データセグメント |
| – | name / producers / 他 | 0(custom) | – | デバッグ名等のメタ情報 |

※ START(id8) と DataCount(id12) はこのモジュールには無い。

## 2. 「宣言」と「実装」は別セクションに分かれる

自前定義関数は、**宣言（型）と実装（本体）が別セクションに分割**されている。

```
FUNCTION セクション(id3)        CODE セクション(id10)
┌─────────────────┐          ┌──────────────────────┐
│ 定義関数#0 → typeidx │  ←同じ→  │ 定義関数#0 の命令列     │
│ 定義関数#1 → typeidx │  順序   │ 定義関数#1 の命令列     │
│ ...               │          │ ...                    │
└─────────────────┘          └──────────────────────┘
```

- FUNCTION(id3) … 各定義関数が「どの型か」を `typeidx` で並べるだけ。**本体（命令）は持たない。**
- CODE(id10) … FUNCTION と**同じ並び順**で、各関数のローカル変数宣言＋命令列を持つ。

一方 import される関数は、**IMPORT(id2) の中で型まで含めて宣言**される（本体は当然 wasm 内に無い＝外部供給）。
つまり「関数の宣言」は import 分が IMPORT、定義分が FUNCTION、という二箇所に分かれている。

## 3. 関数インデックス空間：import が先頭を占める

wasm の `call`・`ref.func`・テーブル elem・export はすべて**単一の関数インデックス空間**を指す。
その採番規則は仕様で固定されており、

```
funcidx = [ IMPORT の関数 ] を 0,1,2,... と先に採番
          ─────────────────────────────────
          続けて [ FUNCTION/CODE の定義関数 ] を N, N+1, ... と採番
```

本モジュールでの実測（node で読んだ IMPORT 関数）:

```
funcidx 0: import "__wbindgen_placeholder__"."__wbindgen_describe"
funcidx 1: import "__wbindgen_externref_xform__"."__wbindgen_externref_table_set_null"
funcidx 2: import "__wbindgen_externref_xform__"."__wbindgen_externref_table_grow"
  → 関数 import は 3 個。よって自前定義関数は funcidx 3 から始まる
```

実際、CODE 内の最初の定義関数 `__wbindgen_describe_print_volume` は funcidx 3、
本調査で追った `__wasi_fd_write` は funcidx **150**（= 3 + CODE 内 147 番目）だった。

これが「**import はインデックス空間の先頭を占める**」の意味：
import された関数は常に `0..(関数import数-1)` という**最小のインデックス**を取り、定義関数はその後ろにずれて並ぶ。

```
funcidx:  0    1    2  │  3        4    ...   150          ...
         ┌──────────┐│┌───────────────────────────────┐
         │ IMPORT(id2) ││ FUNCTION(id3)/CODE(id10) の定義関数 │
         │  の関数群   ││  __wbindgen_describe_print_volume,  │
         │（外部供給） ││  ..., __wasi_fd_write(150), ...     │
         └──────────┘│└───────────────────────────────┘
            ↑先頭を占有        ↑import を1個消すと、ここ以降が全部 -1 ずれる
```

## 4. だから import の削除は「丸ごと再採番」になる

import を 1 個削ると、その後ろの定義関数の funcidx が全て 1 ずつ繰り上がる。
すると次を**すべて書き換える**必要がある:

- CODE 内の全 `call <funcidx>` / `ref.func <funcidx>`
- ELEM(id9) がテーブルに載せる funcidx
- EXPORT(id7) が指す funcidx
- START があれば その funcidx

これは事実上モジュール全体の書き換えで、binaryen `wasm-opt --remove-unused-module-elements`
が「未参照 import の削除＋再採番」として行う処理に相当する。ただし**未参照であることが条件**で、
cadrum では `__stdio_write` が ELEM 経由でテーブルに載る（アドレス取得済み）ため鎖を dead と証明できず、
import は残ってしまう。

## 5. import が「呼ばれなくても」効くのはどのセクションか

インスタンス化時にホストが解決を要求するのは **IMPORT(id2) の宣言エントリそのもの**であって、
CODE(id10) 内に `call` があるか否かは無関係。実証として、

- `call` を 6 バイトの nop で潰しても IMPORT エントリは残り、`instantiate({})` は `TypeError` で失敗。
- `call` が 0 個でも import を宣言しているだけのモジュールは同じく失敗。

→ よって「未使用 syscall を call の除去で消す」発想は IMPORT(id2) に手が届かず無効。
no-op スタブは、参照先を import ではなく**自前定義関数（CODE 側）**に化けさせることで、
リンカに IMPORT エントリ自体を出力させない（＝先頭インデックス占有が消える）クリーンな解になっている。

## 参考: 確認に使ったコマンド

```
# セクション一覧
out/clang+llvm-18/bin/llvm-objdump.exe --section-headers <file.wasm>
# 先頭バイト（magic/version）
xxd <file.wasm> | head -1
# import の関数インデックス（node）
node -e 'const m=new WebAssembly.Module(require("fs").readFileSync(f)); \
         console.log(WebAssembly.Module.imports(m))'
```
