# Henkan

様々な形式のファイルを変換できるクロスプラットフォーム・デスクトップアプリ（Windows / macOS / Linux 対応）です。  
**Rust + Tauri v2 + React + TypeScript** で構築しています。

## 主な機能

- [x] **Step 1**: Tauri v2 プロジェクトの初期構築
- [x] **Step 2**: 基本 UI（ドラッグ＆ドロップ / 変換キュー）＋ 画像変換・データ／テキスト変換・Markdown 変換
- [x] **Step 3**: 動画・音声変換（FFmpeg 連携）＋ 文書・PDF 変換（Markdown / Pandoc 連携）
- [x] **Step 4**: インストーラー生成・自動アップデート（Tauri Updater + GitHub Actions）

### 対応カテゴリ
- **画像・SVG（内蔵）**: PNG / JPEG / WebP / GIF / BMP / ICO / TIFF / SVG → 各画像形式・PDF（リサイズ・画質設定対応）
- **Excel・データ・文字コード（内蔵）**: XLSX / XLS / ODS ⇔ CSV / JSON / YAML / TOML、UTF-8 / UTF-8 BOM / Shift_JIS / EUC-JP
- **PDF・文書（内蔵 + Pandoc）**: PDF → Word (DOCX) / TXT / Markdown / HTML / ページ分割、複数画像・複数 PDF の結合、Markdown → HTML（内蔵）、Markdown / DOCX / EPUB / HTML / RTF / PDF 等（Pandoc）
- **圧縮ファイル（内蔵）**: ZIP / TAR / TAR.GZ / 7z の相互変換・フォルダ解凍
- **動画・音声（FFmpeg・アプリ内自動ダウンロード対応）**: MP4 / WebM / MKV / MOV / GIF / MP3 / WAV / FLAC / M4A / OGG / Opus（SNS 向け軽量化プリセット対応）

### 同じ形式での再変換・軽量化

- MP4 / WebM / MKV / MOV は、入力と同じ形式も変換先に選べます。「画質・サイズ設定」の「SNS向け軽量化」を選ぶと、最大720pに抑えて再圧縮します。
- MP3 / WAV / FLAC / M4A / OGG / Opus も同じ形式で再変換できます。動画プリセットは音声のみの出力には適用されません。
- 元ファイルは上書きせず、同名の出力がある場合は連番を付けます。再変換によって容量が増える場合もあります。WAV・FLACの再変換は軽量化を保証しません。

### 変換キューの操作

- 個別変換中の「処理中以外をクリア」は、処理中の項目を残します。変換後の結果やエラーは引き続き確認できます。
- 一括変換中は、キューのクリア操作を無効にします。

## 技術スタック

- **デスクトップ基盤**: Tauri v2 (Rust)
- **フロントエンド**: React 19 + TypeScript + Vite
- **自動アップデート**: `tauri-plugin-updater` + `tauri-plugin-process` + GitHub Releases
- **パッケージマネージャ**: pnpm / Cargo

## 開発コマンド

```bash
# 依存パッケージのインストール
pnpm install

# 開発モードで起動（デスクトップアプリが立ち上がります）
pnpm tauri dev

# Rust ユニット・結合テストの実行
cd src-tauri && cargo test

# リリースビルド＆インストーラー作成
pnpm tauri build
```

## 自動アップデートの公開手順（GitHub Releases 連携）

1. **署名用キーペアを生成する**（秘密鍵は絶対に Git にコミットしないでください）
   ```bash
   pnpm tauri signer generate -w ~/.tauri/henkan.key
   ```
   - 表示された **公開鍵（`.pub` の中身）** を `src-tauri/tauri.conf.json` の `plugins.updater.pubkey` に貼り付けます。
2. **更新チェック先 URL を設定する**
   - `src-tauri/tauri.conf.json` の `plugins.updater.endpoints` にある `OWNER/henkan` をご自身の GitHub ユーザー名／リポジトリ名に変更します。
3. **GitHub Secrets を登録する**
   - GitHub リポジトリの `Settings > Secrets and variables > Actions` に以下を登録します：
     - `TAURI_SIGNING_PRIVATE_KEY`: `~/.tauri/henkan.key` の中身
     - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: キー生成時に設定したパスワード（未設定なら空）
4. **バージョンタグを push してリリースする**
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
   - `.github/workflows/release.yml` が自動で Windows / macOS / Linux 向けインストーラーと `latest.json` をビルドし、GitHub Releases にドラフト作成します。
   - Releases 画面で内容を確認して「Publish release」を押すと、インストール済みアプリから自動アップデートできるようになります。
