# セキュリティ

現在は共通runtimeを実装・検証している段階で、サポート対象のruntime releaseはありません。セキュリティ修正はmainへ反映します。

未公開の脆弱性は [GitHubの非公開報告](https://github.com/neknaj/NEPL3/security/advisories/new) を利用してください。再現入力、対象commit、影響、実行環境を添えてください。秘密情報や悪用可能な詳細を公開Issueへ記載しないでください。

将来の外部provider、文法package、NDF入力の信頼境界は [エディタ仕様](doc/spec/08-editor.md) と [交換仕様](doc/spec/09-portability.md) に従います。設計上の要件と実装済みの防御は区別し、状態を [implementation-status.json](implementation-status.json) に記録します。
