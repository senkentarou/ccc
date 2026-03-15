# ccc (cc collaboration)

## Project Overview

Claude Code のチャット履歴を全文ファジー検索し、3ペインTUIでプレビュー・復元できる Rust 製 CLI ツール。

- **ターゲット**: Claude Code を日常的に使うソフトウェアエンジニア
- **ライセンス**: MIT
- **設計書**: `docs/design/DESIGN.md` にプロダクト全体の設計ドキュメントあり
- **機能設計書**: `docs/features/<feature-name>/design.md` に機能単位の設計書を配置

## Tech Stack

| 層 | 選定 | 備考 |
|---|---|---|
| 言語 | Rust (2021 edition) | |
| TUI | ratatui + crossterm | ターミナルバックエンド |
| ファジー検索 | nucleo | helix-editor で実使用中 |
| MD パース | pulldown-cmark | 信頼できない入力にはサイズ上限を設ける |
| クリップボード | arboard | 1Password 社が共同メンテナー |
| JSON パース | serde + serde_json | |
| 日時 | chrono | 0.4.20 以降を使用（RUSTSEC-2020-0159 対応済み） |
| ディレクトリ | dirs | ホームディレクトリ解決 |
| 並列処理 | rayon | セッションファイルの並列パース |
| CLI 引数 | clap (derive) | |
| テスト | cargo test（標準） | |
| Linter | clippy | |
| Formatter | rustfmt | |
| CI | GitHub Actions | clippy + test + build |

### クレート導入時の注意

- `chrono` は必ず最新版を使用する（過去の脆弱性 RUSTSEC-2020-0159 は 0.4.20 以降で修正済み）
- `pulldown-cmark` でパースする入力にはサイズ上限を設け、パーサー DoS を防ぐ
- `serde` は単独メンテナー（bus factor=1）だが代替不可能。依存は許容する

## Architecture

```
src/
  main.rs          # エントリポイント、CLI引数パース
  app.rs           # アプリケーション状態管理、イベントループ
  parser/          # JSONL パーサー
    mod.rs
    jsonl.rs       # セッションファイルの読み込み・パース
  store/           # セッションストア
    mod.rs
    session.rs     # Session / Message データモデル
  search/          # ファジー検索エンジン
    mod.rs
    fuzzy.rs       # nucleo によるファジーマッチ
  tui/             # TUI レイヤー
    mod.rs
    layout.rs      # 3ペインレイアウト
    session_pane.rs
    message_pane.rs
    preview_pane.rs
    search_bar.rs
    keybindings.rs # キーバインド処理
  render/          # マークダウンレンダリング
    mod.rs
    markdown.rs    # pulldown-cmark → ratatui ウィジェット変換
```

### データフロー

1. `~/.claude/projects/<project-path-hash>/*.jsonl` を読み込み
2. JSONL パーサーで `Session` / `Message` 構造体に変換
3. インメモリストアに保持
4. ファジー検索エンジンでスコアリング
5. ratatui で3ペインTUIに描画

## Coding Conventions

- コミット: Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`)
- フォーマット: `rustfmt` に従う。手動ルールは設けない
- Lint: `clippy` の警告はすべて解消する
- エラーハンドリング: `anyhow` / `thiserror` を使い、`unwrap()` は本番コードで禁止。テストコードでは許容
- 命名: Rust の標準規約に従う（snake_case / PascalCase / SCREAMING_SNAKE_CASE）
- `unsafe` 禁止（外部クレート内は除く）
- パブリック API には doc comment を付ける
- lint / format はローカルで明らかに不要な場合スキップ可（CI で担保）

## Testing

- フレームワーク: `cargo test`（標準）
- 方針: TDD（テストファースト）。実装より先にテストを書く
- カバレッジ: 新規コードは原則テスト必須
- テストの粒度:
  - ユニットテスト中心。ロジックを小さな純粋関数に切り出してテストする
  - JSONL パーサー・検索エンジンは特に厚くテストする
  - TUI 描画はスナップショットテスト（`insta` クレート）を検討

## Development Commands

```bash
# ビルド・実行
cargo build              # デバッグビルド
cargo build --release    # リリースビルド
cargo run                # 実行
cargo run -- --help      # ヘルプ表示

# 品質チェック
cargo test               # テスト実行
cargo clippy             # lint チェック
cargo fmt --check        # フォーマットチェック
cargo fmt                # フォーマット適用

# その他
cargo doc --open         # ドキュメント生成・表示
cargo update             # 依存クレートの更新
```

## CI / Quality Gate

GitHub Actions で PR ごとに自動チェック:

```yaml
# .github/workflows/ci.yml で以下を実行
- cargo fmt --check      # フォーマットチェック
- cargo clippy -- -D warnings  # lint（警告をエラー扱い）
- cargo test             # テスト
- cargo build --release  # リリースビルド確認
```

- ローカルでの lint / format は任意（CI で担保するため開発スピード優先）
- pre-commit は使わない

## AI Development Guidelines

- Claude がコードを自律的に書ける粒度で CLAUDE.md を維持する
- 曖昧さがあればコードに書かず、ユーザーに確認する
- テストが通る状態を常に維持する。壊れたテストを放置しない
- 1コミット = 1論理変更。大きな変更は分割する
- クレート追加時は最新バージョンを `Cargo.toml` に記載する
- 新機能の実装前に `docs/features/<feature-name>/design.md` に設計書を作成する
- `docs/design/DESIGN.md` の設計・データモデル・キーバインドを実装の正とする
- セッションファイルは**読み取り専用**。書き込みは一切行わない
