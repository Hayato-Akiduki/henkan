# Henkan

様々な形式のファイルを変換できるクロスプラットフォーム・デスクトップアプリ（Windows / macOS / Linux 対応）です。  
**Rust + Tauri v2 + React + TypeScript** で構築しています。

## 主な機能（開発ロードマップ）

- [x] **Step 1**: Tauri v2 プロジェクトの初期構築
- [x] **Step 2**: 基本 UI（ドラッグ＆ドロップ / 変換キュー）＋ 画像変換・データ／テキスト変換・Markdown 変換
- [ ] **Step 3**: 動画・音声変換（FFmpeg 連携）＋ 文書・PDF 変換（Markdown / Pandoc 連携）
- [ ] **Step 4**: インストーラー生成・自動アップデート（Tauri Updater + GitHub Actions）

## 技術スタック

- **デスクトップ基盤**: Tauri v2 (Rust)
- **フロントエンド**: React 19 + TypeScript + Vite
- **パッケージマネージャ**: pnpm / Cargo

## 開発コマンド

```bash
# 依存パッケージのインストール
pnpm install

# 開発モードで起動（デスクトップアプリが立ち上がります）
pnpm tauri dev

# リリースビルド＆インストーラー作成
pnpm tauri build
```
