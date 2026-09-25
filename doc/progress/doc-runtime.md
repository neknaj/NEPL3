# Doc runtime の段階実装

T07 は進行中。`doc/spec/05-document.md` と `design/forms.json` を最終契約とし、以下の native API ができたことを T07 全体の完了へ読み替えない。

## 現在状態を確認する入口

正本の一覧は `doc/canonical.json`、実装・受入の状態は `implementation-status.json`、
到達条件は `design/tasks.json` を参照する。以下の履歴にある「未完了」「停止」は、
その段階の結果であり、現在の残件を列挙したものではない。

第05章は通常のpage予算でparse/lowerと文書集合のHTML生成が成立し、正本登録済みである。
内容比較、旧見出しalias、17章・23章へのリンクを検査した移行結果はregistryから辿る。
Markdown集合のWork上限はページ追加に対応する別の実行設定であり、単体の性能保証ではない。
source集合の増大については、readerの競合検査とadmissionだけを実行する限定測定を追加した。
parser全体の実時間・実メモリは未測定であり、単体文書の予算適合やこの限定測定から推定しない。
次の性能改善では、以下の測定を基に既存集合の再走査と検証scopeの寿命を調べる。
受入済みcollectorの再admissionは、台帳の一致を確認できる同期readで再利用する。
同期hostもcallback境界ごとの交換検出を通す。SourceStoreを明示準備した経路では、
環境集合の所有付きscopeも再利用する。正常な同期readでは、collectorが保持する
環境検査済みprefixを引き継ぎ、追加sourceだけを検査する。resume経路は残件である。

Sentence consumerの所有移行を実装中である。Docの文章slotは独立Sentenceの
ForeignClosureを保持し、Articleの選択guestとnamespaceを明示して描画する。
Docのportable表現は共有provenanceをDocOwner表へ一度記録し、各DocClosureが
内容digestで参照する。同値の独立storageも同じportable byte列へ正規化する。
Foundationの単独ForeignClosure交換形式は維持する。

`16f6118` の128段落・512文・18,848 bytesの注釈付き入力は、既存の資源上限でHTML生成まで成功した。
parse/lower/prepare/renderの段階測定を `tools/tests/doc_capacity.rs` で行い、
本文とRubyの512件を確認する。prepareのWorkは90,419,873、描画までの累積Workは
92,233,883、累積AllocationUnitsは276,016,281である。AllocationUnitsは論理的な
累積使用量であり、ピーク物理メモリは未測定である。

PageNamespacePlanはページ・member・文書digest・リンク・残る要求を交換する。
受信側で再構築したnamespace proofから全fieldを照合し、出現順序・所属・添付ファイルの
変更や要求の欠落を拒否する。Doc coreの全46試験はnative・WASIで成功した。
旧ページ試験の型エラーは解消し、root検査と選択guestのnamespace検査へ責務を移した。
RenderedWithForeignは描画結果とguestの出現順・embed・HTML範囲を交換する。
受信側hostが再構築した出力との全field比較を行い、codecはcallbackを実行しない。
返却値はrawデータであり、HTML・namespace・assetの検証済み型は各検査で構築する。
正式readerとSentence rendererを使う初回CBOR受信、本文・options・配置の改変拒否、
共有Rubyの各出現とDoc owner、Work・Allocation・Nodes・Depthの上限一致と1不足、
事前取消の2試験はnative・WASIで成功した。独立レビューは初期2試験の実行と、
補強後の試験・仕様の静的確認を行った。
toolsのArticle・namespace・印字等の24試験と通常容量試験3件もnativeで成功した。
Doc HTMLの旧6試験を明示guest adapterへ移行した。backend単体のfixtureと、
toolsの実Sentence reader・rendererを使う統合試験の範囲を区別する。
Doc HTMLの全8試験はnative・WASIで成功し、all-target Clippyも成功した。
独立レビューは8試験を実行し、未解決リンクのpath・fragment保持を補強した最終差分を静的確認した。
workspace全体の試験コードはcompileに成功した。試験実行は82単位まで進み、
745件成功・14件失敗・7件ignoreでtoolsのlib試験に停止した。
失敗14件はcanonical文書試験であり、旧Sentence構文のfixture・正本の移行を継続する。
停止後のworkspace試験は未実行である。
Markdownのpage出力もHTMLと共通のguest収集・namespace検査を利用する。
Docリンク・anchor・参照を独立Sentence内から描画し、共有ownerの出現ごとにmemberを記録する。
ページ依存digestはmemberを含むcontext/2、rendererはpages/5へ更新した。
描画エラーはpageとrootからのDoc slot・Sentence embed経路を保持する。
二段再入、共有出現、前方参照、別ページの同一局所ID、資源上限一致と1不足の
追加3試験はnative・WASIで成功した。独立レビューも3試験を実行した。
canonical関連試験は20件成功・2件失敗・2件ignoreであり、実文書の旧構文が残る。
annotated_pagesの小規模fixtureを独立Sentence構文へ移行した。
nativeは12件成功・1件失敗である。第01章の正本はSentence境界を明示する構文へ移行し、
解析・lowerを通過した。ページprojectionは既定Work100Mで停止しており、全体費用の改善を継続する。
独立レビューは実文書試験を明示除外した12件を実行し、成功を確認した。
同じ12件はWASIでも成功した。実文書試験の除外は実行コマンドに限定し、試験本体を維持する。
Markdownは同一実行内のnamespace準備完了時のUsageを観測し、描画費用を分離する。
HTMLは32ページのsource集合を固定して登録済みにした後、8・16・32ページの合成を測定する。
source数に応じた索引検索課金を固定し、既存の線形上限を維持する。
最終HTML検証は別に計測する。初回source登録を含む処理全体の線形性は未検証である。
Doc構造検査は直前の不変ownerとregistryの検証証明を保持し、同じownerの再走査を削減する。
各closureのguest・environment・深度・source admissionは個別に検査する。
Doc coreの47試験はnative・WASIで成功した。深い共有Originと独立storageのDepth一致、
別ownerの不正Origin、2番目guestの破損、資源上限一致と1不足を回帰試験で確認する。
第01章の本文・Ruby・Anno・節ID・参照先は独立した静的レビューで保持を確認した。
Sentence collectionも同一operation内のowner証明を再利用し、公開の単独lowerは呼出しごとに検証する。
共有storageと独立storageの意味一致・lower部分のWork差・2番目slotのcategory拒否を確認する。
最小のdoc-sentence featureではHTMLモジュールを除外し、sentence-htmlの選択時に有効にする。
第01章の既定Work上限での停止位置はnamespace準備中であり、Markdown描画は未到達である。
明示計測は既存registryの文書集合用予算を使用し、namespace準備Work144,986,392、
projection完了Work145,020,457・累積AllocationUnits125,596,233で本文とリンクの検査に成功した。
Windows debugの参考時間は準備6.338秒、projection完了6.344秒で、解析・lowerを含まない。
通常試験のWork100Mと失敗状態は維持する。この計測は通常予算への適合や全章の性能保証を示さない。
Sentence adapterの6試験と実言語を含む9試験はnative・WASIで成功し、独立レビューも6試験を実行した。
追加試験の成功を全workspaceの成功として扱わない。
第01章のPageSet符号化とdocument・guestのdigest計算を、同じ不変入力・上限で独立測定した。
符号化は27,403,009 Work、68件のdigest計算は70,987,681 Workだった。
これらは別Budgetによる成分測定であり、projection内の区間差分とは区別する。
digest要求索引は反復挿入からbudget付き安定merge sortへ変更し、構築の二乗時間項を除去した。
128・256・512件でWork増加率、順序別のUsage一致、資源上限一致と1不足を検査する。
変更後のdigest測定は70,986,006 Work、projectionは145,018,784 Workである。
Windows debugの参考時間はdigest 3.233秒、projection 5.905秒だった。
索引の一時領域によりprojectionの累積AllocationUnitsは125,597,353へ増加した。
通常Work100Mの試験はnamespace準備中に停止する。値の走査・符号化・hash処理の改善を継続する。
続く内訳調査では、符号化後の557,386 NDF node中、embedが535,461 nodeを占めた。
単一nodeのguestに含まれるsource数の最大値は221、mapping数の最大値は332だった。
parserが受理済みsource/mapのprefixを各guestへ保持する経路に対応する。
先行生成sourceと多段mapを後続Viewが参照できるため、宣言閉包を保持した共有方式を検討する。
batch digestの各nodeで行っていた二重検索は、1回の検索と一致要求の走査へ変更した。
変更後のdigest成分は67,641,758 Work、projection全体は141,653,799 Workである。
今回のWindows debug参考時間はそれぞれ3.250秒・5.931秒で、実時間の改善は未確定である。
wire全70試験はnativeで成功し、batchの9試験はnative・WASI・独立nativeレビューで成功した。
通常Work100Mへの適合は引き続き未達である。
Hello・MiniExpr・compositionのtutorial正本は、独立Sentence境界を明示する構文へ移行した。
外部URLはSentenceのlink、章間参照はDocのlinkへ接続し、本文・Ruby・節ID・順序を保持する。
3章の実原稿を通常予算でMarkdownへ投影し、章間リンク・外部URL・Ruby・RawCode9件の全文を
独立期待値で検査した。対象試験はnative・WASIで各1件成功した。
原稿全文と試験差分の独立した静的レビューを完了した。今回の独立レビューは実行検証を含まない。
第00章の正本も独立Sentence境界へ移行した。本文・Ruby・Anno・節ID・順序を保持し、
通常予算による実原稿のMarkdown投影で、3リストの項目数4・6・14、INV01〜INV14の順序、
InlineCode10件、Rubyをbaseに持つAnno4件を独立期待値と照合した。
対象試験はnative・WASIで各1件成功し、独立レビューでもnativeの1件成功を確認した。
第13章の正本も独立Sentence境界へ移行し、外部リンク2件をSentenceのlinkへ接続した。
本文・Ruby・節の階層・順序を保持した。実原稿の投影試験にはInlineCode33件、
外部URL2件、見出し階層を独立期待値として追加した。
移行直後の通常試験と正式Markdown集合生成は同章のlowerでWorkLimitとなった。
既存の文書集合用予算を明示した測定試験はnative・WASIで各1件成功した。
原稿と試験の独立レビューを完了し、独立native測定でも同じWork値と内容検査の成功を確認した。
lowerは103,388,054 Work、投影は314,974,072 Workで、通常Work100Mへの適合は未達である。
Windows debugの投影参考時間は14.832秒、WASIは23.044秒だった。
AllocationUnitsはnativeでlower 98,149,695・投影266,947,844、WASIで73,985,695・203,120,488となった。
この値は累積資源会計であり、ピーク物理メモリを表さない。
続いてForeignCaptureが不変owner tableの検証結果を保持する構成へ変更した。
registryを不変借用し、変更時はownerを再検証する。guest・environment・SourceAdmission・
現在のDepthは各captureで検査し、非atomic targetの別storageは完全検証する。
初回・warmのWork/Allocation境界、停止後の再試行、registry再検証途中の停止と復帰を確認した。
coreのnative全146件に続き追加試験1件、capture関連10件のnative・WASI試験が成功した。
Doc・Sentence・Math coreのnative全144件と、4 coreのthumbv6m向けcompileも成功した。
独立レビューはcapture module7件と追加1件をnativeで実行し、追加指摘はなかった。
第13章のlowerは69,274,361 Workへ減り、通常予算内で完了する。
投影は314,974,072 Workのままで、通常の投影試験はWorkLimitを保持する。
文書集合用予算による内容検査はnativeで成功した。正式Markdown集合生成は
次の旧構文の `doc/spec/22-external-extensions.nepld` で停止している。
集合生成と生成Markdownの更新は未完了である。
第13章の投影にnamespace準備完了時の観測を追加し、第01章と共通の計測関数で
portable符号化とdocument・guestのdigest計算を個別に実行した。
namespace準備までのWorkは314,892,522、投影完了までのWorkは314,974,072だった。
独立したcold SourceAdmissionによる符号化は69,831,389 Work・215,101,509 AllocationUnits、
そのNDFに対する148件のdigest計算は153,194,743 Work・19,383,984 AllocationUnitsだった。
個別測定は各々のBudgetを持ち、投影全体の区間差分や加算可能な内訳として扱わない。
符号化後のDocValueは1,195,011 NDF nodeを持ち、embeds fieldが1,181,844 nodeを占めた。
native構文nodeが1件のguestにも最大271 source・279 source mapが含まれる。
この測定はportable表現の埋め込み部分を次の調査対象とする根拠であり、
source closureの縮小や通常予算への適合を確認した結果には含めない。
Windows debugの逐次実行で、第01章・第13章の計測試験2件が成功した。
第13章の参考時間は投影18.034秒、独立符号化4.428秒、独立digest計算10.548秒だった。
これは各処理1回の観測値であり、速度改善率の根拠には使用しない。
計測境界と記述の独立した静的レビュー、対象のclippy、fmt、diff検査を完了した。
今回の計測変更についてWASIとworkspace全体の再実行は行っていない。
SyntaxBundleのportable符号化では、guestを処理する前に構築したcanonical順序と
参照対応表をFinish frameへ保持し、同じbundleに対する二回目の構築を除去した。
第13章の投影は314,972,126 Work・266,936,252 AllocationUnitsとなり、
直前の測定から1,946 Work・11,592 AllocationUnits減少した。
この局所変更の効果は小さく、source表現の大きさと通常予算の超過は引き続き残る。
frameの拡大も資源計上し、祖先の対応表を保持する期間の延長を許容する。
累積AllocationUnitsの減少をピーク物理メモリの減少へ読み替えない。
wireのnative全69件とsyntax codecのWASI10件が成功した。独立レビューでもnative10件が成功した。
128子とguestのarena順序変更、Work・Allocationのexact/不足境界、停止後の入力保持を確認した。
wireのclippyとthumbv6m向けcompileも成功した。workspace全体と正式受入は今回再実行していない。
第22章の正本を独立Sentence構文へ移行した。本文・Ruby・節ID・順序を保持し、
正式readerとlowerを通した通常予算のMarkdown投影で、五層の表の列・順序、
六つの分離条件、InlineCode4件、見出し階層を独立期待値と照合した。
native・WASIで各1件成功し、独立レビューでもnative1件の成功を確認した。
対象のclippy、fmt、diff検査も成功した。
正式Markdown集合生成は第22章を通過し、次の旧構文の
`doc/spec/18-html-delivery.nepld` で停止した。生成Markdownの更新は未完了である。
第18章の正本も独立Sentence構文へ移行した。本文・Ruby・Anno・節IDを保持し、
正式reader/lowerによるモデル上で4節・12段落・42文の所属と順序を確認した。
通常予算のMarkdown投影で見出し階層とresource closureのAnnoを検査し、
native・WASIで各1件成功した。原文との独立レビューと独立native1件も成功した。
対象のclippy、fmt、diff検査を完了した。正式Markdown集合生成は第18章を通過し、
次の旧構文の `doc/spec/07-circuit.nepld` で停止した。
第07章の正本を独立Sentence構文へ移行した。本文・Ruby・節ID・文境界・順序を保持し、
正式reader/lowerとMarkdown投影で9節、宣言5項目、elaborationの6段階、
InlineCode12件、NORの5式と状態表記を独立期待値と照合した。
通常Work100Mの投影試験はWorkLimitとなり、失敗状態と上限を維持する。
既存の文書集合用予算を明示する別試験はnative・WASIで各1件成功した。
lowerはこの別試験でも通常予算を使用する。投影は両targetとも116,549,905 Workで、
nativeの累積AllocationUnitsは134,077,642、WASIは102,166,374だった。
Windows debugの投影参考時間は6.06秒であり、通常予算への適合は未達である。
原稿の独立レビューと独立native1件も成功し、対象のclippy、fmt、diff検査を完了した。
正式Markdown集合生成は第07章を通過し、次の旧構文の
`doc/spec/19-html-fragment.nepld` で停止した。生成Markdownの更新は引き続き未完了である。
第19章も独立Sentence構文へ移行し、外部リンク5件をSentenceのExternalLinkへ接続した。
URI・label・本文・Ruby・文境界を保持し、正式reader/lowerで18段落102文の所属と順序、
Markdown投影でInlineCode17件・外部リンク5件・単一H1を独立期待値と照合する。
初回試験は旧linkのexternal headを拒否し、修正後は通常Work100Mの投影で停止した。
通常試験と上限を維持し、既存の文書集合用予算による別試験はnative・WASIで各1件成功した。
lowerは両試験とも通常予算である。投影は187,567,897 Workでtarget間に一致し、
累積AllocationUnitsはnativeで180,235,993、WASIで137,374,225だった。
Windows debugの投影参考時間は8.53秒であり、通常予算への適合は残る。
独立レビューでも初回のlink拒否を確認し、修正後の独立native1件と静的再レビューが成功した。
対象のclippy、fmt、diff検査も成功した。正式Markdown集合生成は第19章を通過し、
`doc/spec/12-model-invariants.nepld` の旧構文で停止した。生成物の更新は未完了である。
第12章の移行準備で、論理モデルと第05・23章に旧Doc Sentence所有の契約が残ることを確認した。
`interfaces/model.json` の旧文章内8型を除去し、SentenceのcontentとDoc参照のlabelを
構文閉包または型付きSentence値として保持する。Sentence rootとInline rootの制約を第12章へ明記した。
操作対応表の廃止Doc readerを独立Sentenceのliteral操作へ訂正し、本文の旧payload変換手順も更新した。
論理モデルとportableモデルのSyntax/Value、field名・型、reader・payload参照を横断する試験を追加した。
型参照試験7件とconsumer試験native・WASI各6件が成功した。独立レビューは新試験1件を実行し、
field型の期待値を補強した最終差分を静的に再確認した。主担当も補強後の1件を再実行した。
repository check、toolsのclippy、fmt、diff検査を実行した。第05・12章の全文構文移行と
生成物更新は未完了であり、今回の契約検査は両章の正式readerによる完走を保証しない。
canonical encoderの作業stackは論理容量を保持し、容量の増加分だけを事前に課金する。
取り出したslotとhash終了markerのslotを再利用し、各pushのWork・深さ・出力の課金を維持する。
第13章の投影は314,972,126 Workで不変、累積AllocationUnitsは266,936,252から
247,434,268へ減少した。通常Work100Mへの適合は未達である。
独立したCBOR期待値、digest、各資源の上限一致と1不足、広い入力の早期停止を検査した。
wireのnative全71件、encoderのWASI11件、独立レビューのnative11件が成功した。
thumbv6m向けcompile、wireのclippy、fmt、diff検査も成功した。
第13章の文書集合用予算による計測試験はnativeで1件成功した。
AllocationUnitsは論理的な累積量であり、今回ピーク物理メモリと速度改善率は測定していない。
workspace全体、生成物の一致、正式受入の再実行は未完了である。
source一覧のcanonical整列は、件数の二乗を一括課金する処理から、比較・交換前に課金する
in-place heapsortへ変更した。IDの比較長も課金し、既整列入力は隣接検査で順序と一意性を確認する。
ID・revision・digestの順序と、同じID/revisionの拒否を維持する。
件数128・256・512、昇順・逆順・回転順、ID長、空・単一・奇数件のUnicode入力、
Workの上限一致と1不足、停止状態の維持を検査した。native全74件、WASIの追加3件が成功した。
第13章の投影は318,078,658 Workで、文字列比較の課金を含め直前から3,106,532増加した。
累積AllocationUnitsは247,434,268で不変である。この変更は整列中の停止と資源計上を改善し、
同原稿の通常Work上限超過は引き続き残る。source表現の重複とdigest計算の費用も継続課題である。
wireのclippy、thumbv6m向けcompile、repository check、fmt、diff検査を完了した。
独立レビューは最終の追加3件を実行し、通常予算の未達を含む記録を確認した。
第13章の147個のroot guestについて、native source一覧と生成済みportable fieldを測定した。
source出現数は23,762件、完全identityで区別したsourceは271件である。
本文byte数は重複込み5,012,911、重複除去後35,294だった。root guestのmappingは24,528件である。
各root SyntaxBundleのfield部分木を合計すると、sourcesは166,481 node、tokensは577,617 node、
mappingsは392,595 nodeだった。tokensの内訳ではpayloadが318,248、viewsが235,870を占めた。
これらはportable値のnode数であり、Budgetの累積Nodesとは区別する。root fieldの部分木には
その内部の埋め込みを含む。native source一覧の集計はroot guestだけを対象とする。
計測用の走査は符号化時間・digest時間の測定区間外で実行し、productionのUsageへ加算しない。
この結果からsource本文に加え、token payload/viewとmappingを共有化の評価対象とする。
未参照に見えるsourceの削除による縮小は実施していない。通常予算への適合は引き続き未達である。
第01章の67個のroot guestでも同じ計測を行った。sourceは8,751出現・221種類で、
本文byte数は重複込み1,306,602、重複除去後20,730だった。portable fieldのsourcesは
61,324 node、tokensは183,090 node、mappingsは242,259 nodeとなった。
投影は142,716,827 Workであり、この章も通常Work100Mを超える。
両章の明示計測試験はnativeで各1件成功した。独立レビューは第13章を1件実行し、
集計範囲とUsageの区別を確認した。対象clippy、fmt、diff検査も成功した。
WASIとworkspace全体はこの計測追加では再実行していない。
共有provenance表の設計レビューでは、共有保存と各bundleの宣言scopeを分離し、
opaque payloadの走査によるsource削除を行わず、交換単位が共有表を明示所有する条件を確認した。
source/map共有後もtoken payload/viewの費用は残る。共有表schemaとDocへの接続は未実装である。
前段としてSourceStoreへ完全なSourceRefをBudget付き索引で解決するAPIを追加し、wireのSpan復元へ接続した。
source/revision検索とdigest照合を事前課金し、scope外・digest不一致は未解決として返す。
source数1・128・512、上限一致と1不足、取消、割当なし、store不変を検査した。
構造decode分だけのWork予算でSpan復元が停止する回帰も追加し、未課金検索への退行を検出する。
core契約31件とwire全74件がnativeで成功し、追加後のwire source4件はnative・WASIで成功した。
core追加1件もWASIで成功した。独立レビューはcore1件・wire source3件を実行し、
後から追加したwire回帰を静的確認した。core/wireのclippy、thumbv6m向けcompile、fmt、diff検査も成功した。
第13章の明示計測はnative1件成功し、投影は319,330,279 Work、247,434,268 AllocationUnitsとなった。
参照検索の追加課金でWorkは1,251,621増加した。通常Work100Mへの適合と共有表導入は継続する。

Foundationの共有交換形式として `SyntaxBundleSet` を追加した。source/mapの内容表と、
各root bundleの参照集合・構文本体を保持する。共有表をambient sourceへ登録せず、
各memberが選択した宣言だけで既存decoderと構造検査を実行する。nested Foreignは
従来の自己完結したbundleを維持する。schema・生成descriptor・第02章の契約を更新した。
複数entryの整列・重複・未使用、31/32/33 byte参照、隣接memberのscope隔離、
encode/decodeの資源上限一致と1不足、取消・depth復帰を含む追加6試験はnative・WASIで成功した。
独立レビューも6試験を実行し、追加の阻害指摘はなかった。wire全81試験、Clippy、
thumb向けcheck、repository checkが成功した。正式受入は未実行である。
初期encoderは通常のNDF bundleを構築してから共有表へ集約するため、構築中の重複費用が残る。
Doc生成への接続、通常予算での全章処理、生成Markdownの更新は継続作業である。

共有形式を `FoundationValueCodec::encode_syntax_set` / `decode_syntax_set` へ接続した。
入力はbundleの不変借用であり、呼出し側で交換単位を組むためのbundle複製を要求しない。
byte codecと値codecは同じ構築・復元処理を使用する。typed境界もschema検査を行い、
ambient sourceをmemberの不足宣言へ補完しない。追加試験を含む7件をnative・WASIで実行し、
独立レビューも7件を実行した。
第13章の147 root bundleを共有形式へ変換したnative debugの明示計測は675,511 NDF節点、
160,084,088 Work、237,215,742 AllocationUnits、約4.27秒となった。
同じ入力の現行PageSet全体の構築は72,935,975 Work、214,929,421 AllocationUnits、約2.93秒である。
両者は交換範囲が異なる。共有形式はroot bundleだけを扱い、Doc構造・owner等を含まない。
それでも現行の全体構築より高い費用を要するため、出現ごとの内容ハッシュと通常bundleの
中間構築を改善してからDoc portable表現へ採用する。今回の測定を全体性能の改善とは扱わない。

共有表の構築をnative宣言の索引へ変更した。sourceは完全identityと内容、mappingは
両端の完全spanとkindで同値性を確認し、異なるentryを一度だけ構築・ハッシュする。
memberの閉包検査は維持し、rootのsource/map中間NDF列を省いた。nested Foreignは通常形式を保持する。
従来の単独bundle codecと試験側の内容表から構築した独立期待値に対して、空集合・反復・
異なるmember・同値の別storageでbyte一致を確認する。逆順64/128/256宣言の増大試験も追加した。
第13章の同じnative debug計測は124,476,780 Work、117,349,413 AllocationUnits、約1.52秒となった。
NDF節点数675,511は不変である。これは共有形式の構築部分の改善であり、現行Doc生成経路は未変更である。
独立レビューは8試験を実行し、別storageと増大試験を追加した最終差分を静的確認した。
最終版はwire全84試験とWASIの共有形式9試験、Clippy、thumb向けcheck、repository checkが成功した。

Docのportable交換へ共有形式を接続した。DocValueはsyntaxSources/syntaxMapsを所有し、
各DocClosureはSharedSyntaxBundleを保持する。nativeモデルには表を複製せず、
既存のFoundationValueCodecを使用する。元EmbedRef順、owner scope、member scopeを保持し、
復元後も従来のForeignClosure検査を行う。document/guest digest domainをv3へ変更した。
Syntax/Value混在、別bundle、単独/集合embed値一致、初回受信と再符号化、非空mapping、
欠落・余剰・改変した表、旧field数の拒否を追加試験で確認する。
Doc core全48試験はnative・WASI、toolsのprepare24試験とfeature有効のsuite consumer6試験はnativeで成功した。
独立レビューはprepare12試験を実行した。Clippy、thumb向けcheck、repository checkも成功した。
第13章の投影は336,411,557 Work、162,972,497 AllocationUnits、native debugで約8.84秒となった。
共有化前の319,330,279 Workに対してWorkは増加し、通常Work100Mへの適合は未達である。
DocValueは1,195,011から691,029 NDF節点へ、独立した148件のdigest計算は153,194,743から
75,756,838 Workへ縮小した。索引構築費用の削減、残る正本移行と生成Markdown更新を継続する。

mappingの索引構築では、完全SnapshotIdをsource表へ照合してから、表内位置・byte範囲・
kindを一時的な整数キーとして使用する。位置は交換形式へ出さず、元のMappingから
内容digestとcanonical値を生成する。memberの宣言順序も保持する。
revision・範囲・kindの相違とsource表内位置の変化を独立oracleへ照合した。
wire全85試験、WASIの共有形式10試験、Doc prepare12試験が成功し、独立レビューも10試験を実行した。
Clippy、thumb向けcheck、repository checkも成功した。
第13章の投影は308,211,465 Work、164,738,513 AllocationUnits、native debugで約8.62秒となった。
Workは28,200,092減少し、一時的な整数キーの保持によりAllocationUnitsは1,766,016増加した。
通常Work100Mへの適合は継続課題である。
さらに、不変借用した同一SnapshotIdの比較を1 Workで確定する。別storageのidentityは
完全比較し、addressは交換順序・内容digestへ使用しない。停止・取消・異なるrevisionと
別storageの同値をunit試験で確認した。wire全86試験、追加unitのWASI実行が成功し、
独立レビューはunitと共有形式の計11試験を実行した。
第13章の投影は300,237,833 Work、164,738,513 AllocationUnits、native debugで約8.88秒となった。
Workは7,973,632減少した。単回測定の実時間は改善しておらず、通常Work100Mへの適合も未達である。
共有表のsource・mapping参照検索も、一致位置の確定後に同じキーを再比較しない共通処理へ統一した。
存在・不在と比較回数の境界を追加し、wire全87試験、WASIのpool試験2件、独立レビュー12試験が成功した。
第13章の投影は300,042,375 Workとなり、削減は195,458 Workに留まる。
AllocationUnitsは164,738,513のままで、native debugの単回測定は約8.94秒であった。
残る主な調査対象はtokenのpayload・viewを含むencodingとcanonical digest計算である。
構造別計測により、129件のSentence literalが11,743個のViewElementをpayloadとtokenへ
二重保存していることを確認した。literal専用のSentenceLiteralViewをowner・head・viewDigestとし、
受信APIへtokenのViewBundleを明示的に渡す。ownerのみのscopeで完全Viewを検査・hash照合し、
復元後のliteral検査も維持する。一般SentenceSyntaxの表現は変更しない。
異なる妥当View、別token、zero-width View、31/33byte digest、旧View表現と資源停止を検査した。
独立レビューで既存Doc試験の追従漏れ2件を修正し、独立Sentence試験19件と修正後Doc試験10件が成功した。
Sentence全57試験、host5試験、Doc payload10試験はnative/WASIで成功し、suite統合6試験はnativeで成功した。
Clippy、thumb向けcheckとrepository checkも成功した。正式受入の完了を示す結果には含めない。
第13章の投影は286,046,899 Work、159,308,444 AllocationUnits、native debug単回で約6.39秒となった。
独立した148件のdigest計算は75,756,838から51,363,577 Workへ減少した。
通常Work100Mへの適合、未移行正本と生成物の更新は引き続き未完了である。
共有syntaxのnative検証を独立計測すると19,657,161 Workであった。共有表を構築する前に、
同じ不変snapshot storageの重複を集約し、完全identityの整列対象を削減した。
索引整列の課金を固定し、最初の出現順を復元することで、address配置が後段の比較費用へ影響する経路を除いた。
別storageは保持し、memberごとの検証と内容競合検査を従来通り実行する。
wire全88試験、WASIのpool試験3件と共有形式10件、Clippy、thumb向けcheck、repository checkが成功した。
独立レビューでもpool試験3件と共有形式10件が成功した。URI競合の拒否型に関する試験の期待値を修正した。
第13章の投影は256,474,813 Work、159,712,398 AllocationUnits、native debug単回で約6.53秒となった。
Workは29,572,086減少し、集約用索引によりAllocationUnitsは403,954増加した。
通常Work100Mへの適合は未達であり、native検証・変換・digestの費用を継続して削減する。
Doc encodingでは、直前の文書全体検証の結果とregistryを非公開EncodingInputへ保持し、
owner表の生成へ渡す。owner・各closureの再検証を除き、最深出現位置・source・environmentの
検査結果を同じ操作・Budget・admission内で再利用する。外部入力とFoundation codecの検査は維持する。
不正なowner environment・guest root・置換ownerをportable入口からも拒否する試験を追加した。
Doc core全48試験がnative/WASIで成功し、独立レビューはforeign・portable・prepareの22試験を実行した。
hostのdoc_prepare24試験、Clippy、thumb向けcheck、repository checkも成功した。
第13章の投影は236,514,055 Work、156,120,521 AllocationUnits、native debug単回で約6.51秒となった。
encoding単独は80,153,878 Workである。投影全体の通常Work100Mへの適合は引き続き未達である。
mapping端点のsource解決には、digest・revision・source名の順に完全identityを照合する
副索引を追加した。固定長fieldから検索し、長いsource名の反復比較を減らす。
元のsource pool位置・交換形式・memberごとの権限検査を維持し、同digestの別sourceと
別revisionも完全照合する。索引構築・検索・確保は同じBudgetへ課金する。
第13章の投影は230,025,005 Work、156,122,689 AllocationUnits、native debug単回で約6.36秒となった。
直前の実装から6,489,050 Workを削減し、索引によりAllocationUnitsは2,168増加した。
同storageのpointer索引も試験したが、同原稿ではWorkが増加したため撤回した。
採用した内容索引ではwire全90試験がnative/WASIで成功し、独立レビューでもpool5件と
共有形式10件が成功した。Clippy、thumb向けcheck、repository checkも成功した。
通常Work100Mへの適合と全文書生成は未完了であり、この局所改善を正式受入へ昇格させない。
memberのsource参照生成も同じ副索引へ接続し、位置の正準整列とdigest列の生成処理を共有した。
第13章の投影は227,798,829 Workとなり、AllocationUnitsは156,122,689のままである。
native/WASIのwire全90試験、独立レビューの共有形式10試験、Clippy、thumb向けcheck、
repository checkが成功した。独立した形式oracleとmemberの権限境界の試験を維持している。
投影hostへ段階観測を追加し、同じ操作の累積Usageと実時間を測定した。第13章では
Discoveryが84,678,623 Work、namespace検査の追加分が20,594,917 Work、
namespace解決・encoding・digest計算の追加分が122,443,739 Work、最終投影が81,550 Workであった。
native debug単回の累積実時間は順に約1.98秒、2.18秒、6.27秒、6.27秒である。
通常入口との出力・Usage一致、通知順序、Discovery直後の停止で後段を完了通知しないことを
native/WASIで確認し、独立レビューでも同じ試験が成功した。時刻と計測結果は成果物へ含めない。
次の性能改善では検証・変換・digest計算を優先し、最終Markdown出力の費用と区別する。
Spanはpointer atomicsを持つtargetで不変SnapshotIdを共有し、source名の反復複製を削減した。
非atomic targetは従来のowned表現を維持する。snapshot取得は独立した値を返し、
Spanの等値・範囲・寿命とportable形式は維持する。identityの追加共有領域はsource作成時に課金し、
実複製用のSpan::clone_with_budgetをSentenceの位置保持へ接続した。独立値の比較用課金は維持する。
第13章のlowerは39,722,678 AllocationUnits、投影は153,021,007 AllocationUnitsとなった。
投影Workは227,777,241、同時検証終了後のnative debug単回は約6.38秒である。
この測定は主に複製費用の改善を示し、実時間の短縮や通常Work100Mへの適合を示す結果には含めない。
core・wire・Sentenceのnative/WASI各296試験、独立レビュー61試験、Clippy、thumb向けcheck、
workspaceの`--all-targets` check、repository checkが成功した。reader・engine・Doc coreの
native206試験も成功した。正式受入の状態は変更していない。
SnapshotIdの完全比較を共通APIへ集約し、同じ不変参照は1 Workで照合する。
独立storageにはsource名とrevision・digestの比較費用を課金する。mapping graphの
直前位置・隣接位置・二分探索とwireのsource poolがこの比較を使用し、元の順序と権限境界を維持する。
共有storageと独立storageから構成した同一graphの一致、cycle拒否、identity全field、
exact・不足・取消を確認した。native/WASI各241試験、独立レビュー64試験、Clippy、
thumb向けcheckが成功した。第13章のlowerは64,078,617 Work、投影は218,934,368 Workとなり、
投影AllocationUnitsは153,021,007のままである。native debug単回は約6.27秒であった。
通常Work100Mへの適合と全文書生成は引き続き未達である。
source poolの二次索引でも同一の不変identityを1 Workで照合する。別storageには
digest・revision・source名の比較を適用し、正準順序と宣言範囲を維持する。
wireのnative/WASI各91試験、独立レビュー18試験、Clippy、thumb向けcheck、repository checkが成功した。
第13章の投影は215,506,636 Workとなり、前段から3,427,732 Work減少した。
AllocationUnitsは153,021,007、native debug単回は約6.32秒であり、実時間の改善は確認していない。
captureはownerの由来とguestの明示宣言を保持する。任意payload内のsource参照を扱う契約を
追加せずに宣言を削減する処理は採用していない。通常Work100Mへの適合は継続課題である。
単独Article・Sentence・Inlineのprepareでは、文書とregistryを不変借用するEncodingInputを
ラベル検査とportable化で共有し、native構造検査の反復を除去した。構造・ラベル・符号化・
出力schema検査の順序と構造エラーの分類を保持する。3種類のrootで従来経路とのdigest一致、
WorkとAllocationUnitsの削減、exact・不足境界、owner改変後の拒否を確認した。
Doc coreのnative/WASI各49試験、host準備24試験が成功した。独立レビューはprepare/portableの
20試験と最終API整理の静的確認を行い、追加指摘はなかった。Clippy、thumb向けcheck、
repository checkも成功した。この再利用は単独prepareを対象とし、ページ集合投影の削減値には含めない。

ページ集合のMarkdown生成には、構造検査・ラベル収集・符号化・namespace解決を同じBudgetと
source admission内で実行する `pages::namespace::with_resolved` を接続した。完成した検証値を
callbackへ借用させ、操作内のmember表を保持する。各呼出しで検証を実行し、guestの相対深度、
重複した表示上の出現、完全な出力schema検査を維持する。別Budgetへ検証結果を持ち越すAPIは設けない。
従来経路とのportable plan全体の一致、Work・Allocation削減、各資源の不足境界、callbackの
失敗と取消しを確認した。Doc coreはnative/WASI各51試験に成功し、独立レビューもnamespacesの
8試験とhost差分を確認した。host試験で検出したnamespaceエラー表示の回帰を修正し、関連試験を再実行した。
WASIのhost選択試験は19件成功・4件ignoreである。既知のWorkLimitとなる第13・19章の2件を除外し、
4件のignoreは明示実行用の章別計測である。Clippy、thumb向けcheck、repository check、fmt、diff検査も成功した。

第13章の投影はWorkが215,506,636から196,344,754、AllocationUnitsが153,021,007から
149,323,420へ減少した。単独debug実行の投影時間は約5.97秒だった。計測stageのInspectionをSelectionへ
変更し、構造検査をResolution内へ統合したため、旧stage単独の費用とは直接比較しない。
通常上限100,000,000を維持したhost試験では第13・19章がWorkLimitとなる。この2件とcanonical生成物の
更新は継続課題であり、今回の改善による通常ページ予算への到達は未完了である。

第12章の正本を独立Sentence構文へ移行した。本文・Ruby・コード・参照を保持し、Docの文章slotと
Sentence内部の構築、Doc固有の相対リンクの境界を明示した。独立レビューは旧原稿との全引用文字列の
内容・順序一致を確認した。正式reader/lowerとMarkdown解析で7節・32段落の文境界、制約表17項目、
InlineCode9件、相対リンク1件を検査する。通常Work100Mの投影試験は停止を継続し、既存の文書集合予算による
明示試験はnative/WASI各1件と独立native1件に成功した。両targetのWorkは137,729,143、累積AllocationUnitsは
nativeが156,176,112、WASIが114,371,540だった。lowerは通常予算を使用する。
Clippy、repository check、fmt、diff検査も成功した。正式Markdown集合生成は第12章を通過し、
次の `doc/spec/08-editor.nepld` の旧構文で停止した。生成Markdownの更新と通常ページ予算への適合は継続する。

第08章の正本にも独立Sentenceの境界を追加した。独立レビューで、差分は285個のsentence挿入に限定され、
引用文字列の内容と順序、10節・58段落を保持することを確認した。正式reader/lowerで各段落の文境界を検査し、
Markdown解析で11個のInlineCode、Ruby、不要な表・リスト・リンク・コードブロックの不在を確認する。
lowerは通常予算で47,531,397 Workに収まる。projectionは通常Work100Mで停止し、既存の文書集合予算では
native/WASI各1件が成功した。projectionのWorkは両targetとも202,583,499、累積AllocationUnitsは
nativeが279,857,852、WASIが204,782,384だった。通常上限は維持し、その試験も有効のまま残す。
独立native試験も1件成功したが、その時点ではlowerにも文書集合予算を使用していた。最終版ではlowerを
通常予算へ限定し、独立担当はその差分を静的に再確認した。Clippy、repository check、fmt、diff検査は成功した。
正式Markdown集合生成は第08章を通過し、第20章の旧構文で停止した。集合生成物はまだ更新していない。

第20章を独立Sentence構文へ移行し、外部リンク4件はSentenceのExternalLinkへ接続した。
旧local APIのDoc所有モデルと、開発hostによる独立Sentence・再帰Doc Inlineの合成を本文で区別した。
hostのpage登録、namespace解決、追加guestのadapter要求も現行実装へ対応させた。
独立レビューはこれら3箇所の説明変更をコードと照合し、他の引用文字列・文境界の保持を確認した。
正式reader/lowerとMarkdown解析による16段落・2節・InlineCode14件・外部リンク4件の試験は、
通常予算でnative/WASI各1件、独立native1件が成功した。両targetでlowerは17,135,091 Work、
projectionは85,209,206 Workだった。projectionの累積AllocationUnitsはnativeが105,891,274、
WASIが77,479,222である。既存export試験7件、Clippy、repository check、fmt、diff検査も成功した。
正式Markdown集合生成は第20章を通過し、第17章の旧構文で停止した。生成物の更新と、
他章の通常予算への適合、旧local APIの撤去は継続する。

第17章の正本を独立Sentence構文へ移行した。本文・Ruby・表のセル・文境界を保持し、外部リンク5件を
Sentence、相対リンク1件をDocの所有境界へ接続した。正式reader/lowerとMarkdown解析によって、
7節・27個の節内段落・3個のリスト項目内段落、2表の11データ行・30セル、InlineCode21件、
リンクの順序とリスト項目への所属を検査する。通常予算のlowerは58,268,646 Workで成功した。
projectionは通常Work100Mで停止し、同試験を有効のまま維持する。既存の文書集合予算による明示試験は
native/WASI各1件と独立native1件が成功し、projectionのWorkは両targetとも199,142,087だった。
独立レビューは引用値・Ruby・文境界とリンクの所有を確認し、通常予算の停止も再現した。
累積AllocationUnitsはnativeが149,510,418、WASIが108,132,202である。Clippy、repository check、
fmt、diff検査も成功した。正式Markdown集合生成は第17章を通過し、第11章の旧構文で停止した。
生成物の更新、通常ページ予算への適合、文書集合全体の検証は継続する。

第11章の正本を独立Sentence構文へ移行した。独立レビューは167箇所のsentence境界と1箇所のdoc境界の
追加を確認し、引用値・Ruby・受入条件の保持を照合した。正式reader/lowerとMarkdown解析の試験で、
6節・16個の節内段落、13リスト・66項目の文境界、57試験群とS06の9負例の順序、InlineCode24件、
相対リンク1件を検査する。lowerは通常予算で39,071,131 Workだった。projectionは通常Work100Mで停止し、
その試験を有効のまま維持する。既存の文書集合予算による明示試験はnative/WASI各1件と独立native1件が
成功した。projectionは両targetで153,622,659 Work、累積AllocationUnitsはnativeが165,717,801、
WASIが121,144,757である。Clippy、repository check、fmt、diff検査も成功した。
正式Markdown集合生成は第11章を通過し、第06章の旧構文で停止した。集合生成物の更新と通常ページ予算への
適合は継続する。この原稿試験は記述された57試験群の正式受入を実行するものではない。

第06章の正本を独立Sentence構文へ移行した。独立レビューはsentence223箇所とdoc1箇所の追加を確認し、
全引用値・Ruby・Anno・文境界の保持を照合した。正式reader/lowerとMarkdown解析で10節・55段落、
評価規則13項目の順序と所属、InlineCode12件、自由記号の注釈、相対リンク1件を検査する。
lowerは通常予算で45,717,812 Workだった。projectionは通常Work100Mで停止し、通常試験を有効のまま維持する。
文書集合予算による明示試験はnative/WASI各1件、独立native1件が成功した。projectionは186,263,920 Work、
累積AllocationUnitsはnativeが211,954,268、WASIが154,271,640である。Clippy、repository check、fmt、
diff検査も成功した。正式Markdown集合生成は第06章を通過し、第16章の旧構文で停止した。
通常予算への適合と集合生成物の更新は継続する。この原稿試験はMathの意味処理の正式受入を実行するものではない。

第16章の正本を独立Sentence構文へ移行した。独立レビューはsentence178箇所・doc2箇所の追加と、
全引用値・Ruby・移行条件の保持を確認した。readerが検出した3文の移行漏れを修正し、再レビューした。
構造・表示試験は7節の親子関係、29個の節内段落、手順リスト2件・9項目、表1件・12セル、
InlineCode30件、リンク2件を検査する。通常試験はparse後の構文木検証でWork100Mに達し、
lowerとprojectionへ進まない。解析だけ明示上限を選んだ診断でも、lowerが通常Work100Mで停止した。
各段階へ既存の文書集合上限を明示したignored計測はnative/WASI各1件、独立native1件が成功した。
parseと検証は111,311,847 Work、lowerは108,677,967 Work、projectionは329,875,518 Workである。
累積AllocationUnitsは順にnativeで184,390,035・63,205,672・230,560,702、WASIで
132,901,183・45,934,476・166,260,486だった。通常試験とproductionの上限は維持し、
通常予算への適合を性能改善の残件として扱う。Clippy、repository check、fmt、diff検査は成功した。
正式canonical集合生成も第16章の構文木検証でWorkLimitとなった。canonicalのsource名を用いる
parseは86,302,966 Workであり、試験用source名の計測と区別する。生成物の更新は保留する。

第16章の停止を調査し、SyntaxBundleとowner provenanceのsource登録で、重複確認と挿入が同じ索引を
二度検索する経路を除去した。admissionを先行させ、内容競合・重複宣言の拒否、宣言順序、停止前の状態保持を
維持する。追加2試験はWork/Allocationの不足境界を走査し、逆順128宣言でも結果を比較する。
構文検証単独のWorkは30,585,274から19,855,646、構文木検証全体は32,590,339から21,860,711へ減少した。
独立したBudgetで各公開検証APIを測る診断試験を追加した。分解値の合計を実行receiptとして扱わない。
第16章は通常予算でparse・検証89,852,591 Work、lower76,489,083 Workとなり、両段階を通過した。
projectionは329,875,518から275,563,786 Workへ減少したが、通常Work100Mの試験は引き続き停止する。
明示上限の内容保持試験はnative/WASI各1件が成功した。core全体はnative/WASI各153件、独立nativeは
追加unit・syntax・store計31件が成功し、追加指摘はなかった。Clippy、repository check、fmt、diff検査も成功した。
正式canonical集合生成は第16章を通過し、第03章の旧構文で停止した。生成物の更新と通常projection予算への適合を継続する。

旧local描画・portable APIと利用試験、原稿・fixture・生成物の移行、portableページ描画、Mathを含む全体namespaceの接続を
継続する。T07/T21と正式受入は引き続き未完了である。

## 段階別の履歴

### 2026-09-21: collectorの環境検査済みprefixを保持

正常な同期readが返すcollectorに、開始時のsource長までの競合検査scopeを保存する。
新規生成sourceは次のreadで検査する。公開appendは追加分をこの証明へ含めず、
checkpointとrollbackは元の検査範囲を実source列と一緒に保持する。
別store・未準備store・raw復元・resume・停止ではこの省略を推定しない。
SourceChecksの補助cacheは検査済みの値列であり、collector内のindexではない。

限定測定`accepted_prefix_growth_measurement`はsourceを1件ずつ増やして競合検査だけを
128/256/512回行う。source構築・parser・admissionは含めない。
従来のWorkは8,384/33,152/131,840、prefix保持時は10,797/21,677/43,437だった。
128件では定数費用によりWorkが増えるが、増大入力の反復比較は線形に変わる。
Windows debugの参考時間は従来0.26/0.78/3.02 ms、prefix保持時0.046/0.073/0.147 ms。
これは全parserの実時間や実heapの改善率ではない。別分岐が同じ長さの場合、rollback後の
別suffix、環境競合、検査停止、実host生成直後の未検査suffixを回帰試験で確認した。

### 2026-09-21: 変更されていないSourceStoreの環境比較を再利用

SourceStoreのscopeは集合を構築した後に明示準備する。新規挿入と非空編集のcommitで
失効し、同値の重複挿入・空編集・途中停止では維持する。再準備しない場合と非atomic
targetは従来の内容比較を使う。台帳のscopeとは別であり、schemaや資源入場を証明しない。
共通開発hostのparse入口で準備し、tokenizerは同じ集合の反復比較だけを省く。

限定測定`environment_scope_growth_measurement`は128/256/512件の環境をそれぞれ
127/255/511回再比較する。初回の保存とfixture構築、parser、accepted sourceは含めない。
未準備時のWorkは16,256/65,280/261,632、準備時は0であり、各回のBudget pollは維持する。
Windows debugの参考実時間は未準備0.38/1.42/5.85 ms、準備0.011/0.016/0.033 msだった。
この測定は環境比較だけのもので、parser全体の性能・実heap削減・全source経路の線形化を
示さない。AllocationUnitsは実heap計測ではない。独立レビューで求められた編集commit
直前のAllocation停止も、集合とscopeの不変性を回帰試験で確認した。

### 2026-09-21: source集合の反復走査の基準測定

`tokenizer::source_checks::tests::source_scope_growth_measurement` は明示実行するignored testである。
固定32 sourceと、読取りごとに1 source増える入力を128/256/512回処理する。
測定対象のaccepted sourceは各1 KiB、固定environmentの32 sourceは各4 bytesである。
fixture構築・parser・I/Oを測定に含めない。
同じscope内で実際のSourceChecksとSourceAdmissionを再利用する。

| 増大入力の読取り回数 | admission走査回数 | 競合検査Work | admission Work |
| ---: | ---: | ---: | ---: |
| 128 | 8,256 | 32,960 | 144,462 |
| 256 | 32,896 | 82,304 | 477,190 |
| 512 | 131,328 | 230,144 | 1,714,046 |

Windows native releaseの一回の実測では、この二段階の合計時間は約0.17/0.57/2.25 msだった。
細粒度timerの費用を含む参考値であり、安定した性能閾値や処理系全体の二次時間を主張しない。
固定入力のadmission走査は4,096/8,192/16,384回である。
AllocationUnitsも出力するが契約上の課金値であり、実heap使用量ではない。
測定testはsource bytesの二重課金がないことも検査する。

この経路には増大する既存集合の二次的な走査が残る。検証省略の実装はまだ行っていない。
AcceptedTokenizationReportのscopeと長さだけでは、現在のSourceAdmissionでの受入済みを証明できない。
集合の分岐・rollback・別admission・環境変更を扱う失効条件を保った上で改善する。

### 2026-09-21: 同期collectorのadmission再利用

上記測定後、atomic pointerを持つtargetで、hostなし同期readが返す所有されたcollectorに
SourceAdmissionのopaque scopeを保持する経路を追加した。同じ台帳なら入場済みsourceの
再admissionを走査せず、別台帳では全sourceを検査する。source環境との競合検査は維持する。
checkpointとrollbackは実際のsource集合とscopeを一緒に保持する。
公開append/diagnosticへ別台帳を渡す場合は以前の証明を失効させる。

host callback中の台帳交換をまだ追跡しないため、host付きread・Await・Reserve・resume・
Stopped・raw復元には証明を付けない。非atomic targetも全検査を維持する。
したがって全source処理の線形化、Doc host全体の高速化、実heap削減が完了したとはしない。
次段階はcallback/resumeを含む台帳寿命と、残るSourceChecksの集合比較である。

### 2026-09-21: 同期hostの台帳交換を追跡

同期hostのprovider/reservation callbackを共通wrapperで囲み、各返却時点の台帳を
読取り開始時のscopeと照合する。失効は累積し、複数callbackでAからBへ交換した後に
Aへ戻っても証明を再発行しない。正常終端かつ台帳交換・host errorがない場合だけ、
host付きreadも受入済みcollectorの証明を保持する。

callback内部だけで台帳を交換して元へ戻した場合、返却sourceはruntimeによって
現在の台帳へ再検査・入場される。callback内部の任意状態を証明しているわけではない。
試験は二つのprovider呼出しでsourceを実際に生成し、A→B→Aで失効した後に
最初のsourceの再admissionが必要になることをSourceBytesで確認する。
reservationの交換、None、エラー返却でも失効を累積する。
resume/portableからは引き続き証明を発行しない。

実文書6件のparse/lower/labels試験も成功した。第05章はそれぞれ73,092,384 /
66,285,115 / 8,413,256 Workで、各段階の通常100,000,000 Work枠内だった。
未採用の第21章草案はparseが113,044,768 Workであり、明示した草案用枠での成功を
通常ページ枠への適合や正本移行完了とは扱わない。実時間の比較実験・実heap測定は別途必要である。

## #158を優先するSentence・注釈の回復

今後の是正順序は[23章](../spec/23-sentence-annotation.md)に従う。Doc固有機能を広げる前に、
Sentenceの独立言語化からDoc本文の文章境界を移行し、実文書性能とT19 docs-only Pagesを優先する。
NEPL3a完成・旧comment-as-triviaの全面移行でD本文・公開を止めない。これらは後続で完了させる。
以下の既存API・性能調査はその時点の実装記録であり、旧所有関係を維持する決定ではない。

最初の実装として`nepl3-sentence-core`を追加した。foundationだけに依存する`no_std + alloc`の
順序付き有限arena、category/参照/循環/到達性・非空文章部の検査と独立schemaを持つ。
descriptorは既存生成器で`interfaces/sentence.json`から生成し、実descriptorからidentityを計算する。
通常のURL構造とforeign参照を保持するが、guest実行やDの名前解決、安全なHTMLの認証はしない。

新規10試験は成功した。共有DAGの最長深さ、10万段の非再帰検査、資源停止、foreign参照共有・
未使用拒否、root category、schema所有者・revision・field順の境界を含む。
独立レビューで不足を指摘されたforeign/root試験を追加し、再レビューで指摘なしを確認した。
ARMv6-M、wasm32-unknown-unknown、wasm32-wasip2はbuild確認であり、target上の実行証拠ではない。

続いてSentenceValueのNDF codecを実装した。実descriptorの完全identity、受信前schema検査と
受信後arena検査、foundationによるforeign閉包検査を行う。手組みwire fixtureと実CBOR往復、
共有参照、Unicode、1万段の意味上の深さ、処理途中の非ゼロ予算停止、不正owner source/hashを検査する。
この値codecはSentence局所syntaxの位置情報・reader payloadを完成させるものではない。
codec追加後のSentence全19試験はnativeとWasmtime 44.0.1上のwasm32-wasip2で成功した。
ARMv6-Mとwasm32-unknown-unknownは引き続きbuild検査であり、browser実行は未検証である。
これは同じRust実装のtarget別検証で、独立した第2provider実装の適合を意味しない。
Source/Origin境界の準備中に、値codecで文章とforeignの深さを別々に検査していた欠陥を確認した。
最深の所有位置に親Budget深さを加えてguestを検査するよう修正した。文章42段・guest30段は
単独では上限60に収まるが、合成時は送受信とも拒否する。短経路から先に到達する共有guestでも
最長経路を使い、呼出元の深さを戻す。独立レビューと回帰試験で確認した。

続いてSentenceSyntaxの局所Source/Origin/View境界を追加した。意味arenaと同順・同長の位置表、
必須Origin、任意head/cover、Viewのowner、payload自身のsource/mapを両codec方向で検査する。
架空Spanを要求せず、生成値はSynthetic/Generated Originを使う。これは位置の閉包・関連付けの
検査であり、まだ未実装のreaderによる原文と意味値の一致を証明しない。

独立レビューで、明示SourceMapを持つViewの親子をnativeでは受理しportableでは拒否する欠陥を
確認し、foundation codecとSentence/Doc/Mathのsyntax境界を一緒に修正した。mapを各Viewで
再検査する初版は実14ページ文書でWorkLimitとなった。不変scopeに閉じた検査結果を再利用し、
既存上限のまま生成成功を確認した。各Viewは再検査し、scopeの再束縛とSourceAdmissionへの
可変アクセスでは親子双方のcacheを失効させる。scope越しのadmission置換を含む負例を維持する。
SentenceはCIのWASI実行・Wasm/ARM build対象にも追加した。
最終差分の全workspace試験は730成功・失敗0・既定ignore 2（所有continuation性能比較、
Node/npm依存KaTeX corpus）。Sentence/Wire/Doc/MathはWasmtime 44.0.1で134成功・失敗0・
ignore 0、ARMv6-Mとbrowser向けWasmのbuildも成功した。独立レビューの追加指摘を修正し、
再レビューで指摘なしを確認した。Sentenceのreader/LanguagePackageと注釈・consumer移行、
旧コメントの撤去は引き続き未完了である。

Sentence literalの解析本体と専用印字を独立coreへ移した。Doc型へ依存せず、Ruby/InlineAnno、
escape前のView、元Source/Originとdense位置表を保つ。Unicode escapeの途中、空注釈部、
不正delimiter、1,000段の入れ子、共有DAGの出力膨張、明示windowとCBORの往復を検査した。
literalで表現できないBreak等はNotLiteralとして返し、Textへ黙って変えない。
この追加後のSentence全33試験はnative/WASIで成功し、独立レビューで指摘なしを確認した。
LanguagePackage、reader/provider包絡、prefixと汎用印字は次の実装であり、D本文への接続を
NEPL3a完成より先行する。#165はCIと独立最終レビュー後にmainへ統合済みである。

局所syntaxのSource/Origin境界、literal/prefixと独立LanguagePackage、annotation adapter、
Doc/Math consumerおよび公式sourceの移行は未完了である。旧コメント処理だけは先に削除せず、
移行後にschema・wire・生成器まで撤去する。T07や受入状態をこのモデル検査でpassedにしない。

## 同期host接続と文書移行までの残り

`nepl3_tools::doc::host::NativeHost` は、明示Profileの実装identityと操作登録を
照合してDoc sentence・Name・Number・Trivia providerを実行する。
呼出しに宣言されたsource閉包だけを使用し、同じBudget/SourceAdmissionを保持する。
予約IDは操作内で単調に発行し、停止した予約ではIDを消費しない。
`ParseSession::read_with_host`へ接続することで、各同期callで成長中のparse arenaを
外向けcontinuationへ複写する処理を避ける。providerのreply検査は省略しない。
catalogのVec全体とreply Boxは、実際の確保・provider実行より先に予算計上する。

#158に基づく境界是正では、source hostの環境・reader stateを固定4言語の列挙から
解決済みProfileの登録へ変更した。default categoryとreader modeも解決済みentryを使う。
portable Await経路も同じ登録実装の照合を通し、schemaのpackage文字列によるdispatchを除いた。
追加alias・登録順変更・非root categoryでnative/portableの構文木一致を確認し、
同じ操作schemaでも別implementation digestなら両経路で拒否する。
開発fixtureの言語構成は依然として明示選択であり、一般suite完成やSentence/A抽出、
comment-as-trivia撤去の完了を意味しない。

この接続だけをHTML backendの未保存変更から分離して検証し、nativeのDoc試験32件、
WASI31件が成功した。差はhost processを使用するnative専用seed検査である。
独立レビューでも通常経路・同期経路・fallbackの構文木一致、登録/sourceの不正入力、
予約停止とconstructorの全Allocation上限を確認した。

`linear-combination.nepld`の約13KB全文は、同期接続後もWork上限100,000,000で
停止する。これはHTML出力や文書移行の完成証拠ではない。reader/tokenizerの
継続状態コピーを削減し、同じ入力・予算・位置・診断を用いた回帰検査を進める。
その後、文書間リンクとasset解決、HTML artifact、意味同等性・安定URLの検証を
接続して、準備できたページからnepld正本とPages配布へ進める。

## 現在の実行経路

`nepl3-doc-core` は `no_std` + `alloc`、production 依存は `nepl3-core` だけ。DocValue は型付き arena であり、構造検査・正規化・source/Origin/View 閉包検査と明示 NDF adapter を提供する。深い入力は平坦な参照と反復処理を使い、共有 DAG の最大経路と guest 内部検査の Depth を合成する。

sentence recognizer は元 SourceSnapshot を借用し、Matched / NoMatch / NeedMore / 位置付き Failed を返す。tools の明示 provider がこれを ReadRequest → ReadReply の正式包絡へ接続する。失敗は型付き Diagnostic と Report、停止は元 StopReason を保持する。literal の token payload は `nepl3.doc.DocumentSyntax`、内部 View は外側 prefix parser の子 arity にならない。

正式な Doc/Math/Circuit/Grammar の文法 source から保存した seed を実 compiler へ渡し、4 alias と各 checked environment を明示した Profile で ParseSession を実行する。runtime provider 登録は実 Name/Trivia/Number と Doc sentence のみ。Facts 拡張は compile 時の署名登録であり、未実装の Doc Facts callback を runtime 実装として広告しない。

`lower::prefix` は host が選択を検査した SyntaxBundle と明示 surface SchemaRef/category を受け、現在の Budget/SourceAdmission で再検査して Doc arena へ変換する。構造 proof は PreparedArticle や guest の意味 proof ではない。prefix constructor と sentence recognizer は同じ正規化を使う。実 ParseSession の literal payload と prefix lower の正常例は同じ意味正規形となる。

`lower::document` は同じ変換へ実FoundationValueCodecを渡し、prefixと受理済みSentenceLiteralを一括lowerする。payload自身のsource閉包、外tokenのhead/View完全一致、意味Span/Originのtoken内包含（明示SourceMapを含む）を検査する。元hostのOrigin列と各token-local Viewを維持し、payload Originのみ末尾へ再配置する。各snapshotのSourceBytesを操作内で一度だけ計上し、停止時は入力構文木を変更しない。従来のcodec不要な`lower::prefix`はliteral leafに対して明示Unsupportedを維持する。

補助 constructor は独立 fragment として lower/codec でき、親 operand では型付き enum/Option へ取り込む。取り込んだ wrapper を意味的な子に残さず、元 View/Origin/source 宣言を保存する。Code の DocGuest は `foreign Doc Article` で、同 alias でも独立 bundle/environment を保持する。構文は正しいが注釈が意味的に不正な guest も Code の表示準備のために意味 lower しない。host への復帰を実 parse で検査する。

## 検証の境界

`labels::check` はArticle内のSection/Anchorを先に収集し、前方Referenceを同一DocLabelIdへ解決する。重複名・未解決名と共有DAG上の複数表示経路を型付き失敗にし、名前selectionと定義全体rangeを分離して保持する。source-less位置はNoneのまま、foreign guestのlabelは集計しない。`LabelError::diagnostic` は意味失敗を通常のschema-validated Diagnosticへ変換し、名前と複数経路を構造化引数へ、先行定義をrelated位置へ保持する。構築自身の停止時も元の借用エラーは残る。CheckedLabelsはlabel検査だけのproofで、foreign Requirement解決やPreparedArticleではない。

Doc arena の node/root、Origin、局所 View と source map は portable 往復で保持する。共通 codec は source 表を identity 順、guest syntax graph を正準 NodeRef 座標へ整えるため、未正準な native 表の列順や guest NodeRef 数値そのものの Rust Eq は wire の要件ではない。source の identity/URI/content、owner Origin ID と環境 digest、および実 CBOR の正準再 encode を照合する。

管理対象は Doc core の構造・literal・正規化・source/ForeignClosure・初回 CBOR と、tools の実 compiler/parse/prefix lower・helper 範囲・DocGuest・resource 停止。host seed と元 source/adapter の一致だけは Python process を使う native 専用試験で、同じ保存 seed を使う実処理は WASI でも実行する。browser target は compile 検査であり実描画の成功ではない。

`text::plain_text` は正式PlainTextRequestを受け、document/guestの正準digestへ束縛した明示host textを用いてSentenceを投影する。BaseOnly / WithReadings / WithAllNotesの固定規則と、表示しないnote/readingには未提供embed textを要求しない規則を実行する。全提供entryはpolicyにかかわらず検査する。型付きrequest/replyの初回CBOR、実prefix→lower→投影、元source変更・guest差替え・owner環境変更、準備中と出力中の停止を管理対象で検査する。公開prepareはidentity取得だけで、公開実行が再検査と資源計上を省略するproofではない。これはguest意味checkやPreparedArticleの完成を表さない。

`print::print` はPrintRequest/Replyの正式操作としてPrefix/Compactを出力する。64 form・20 entryは元の表層入力を実parse/lowerし、印字後に同じ実経路へ戻す。MathGuest / CircuitGuest / Guestの独立rootと4 wrapperを保持する。明示host guest-printはdocumentとForeignClosureのdigestへ束縛し、原文取得だけを意味一致proofとして扱わない。名前と言語は共通reader語彙で印字可能性を検査し、source-less不適合は元値を変えず型付き失敗にする。Doc coreはguest意味処理もsnapshot発行も行わない。

## 残り

`prepare::inspect` はArticleの全variant・labelを検査し、Link/Assetの意味node別要求とForeignClosure別要求を列挙する。正式DocPreparationPlanはdocument/guestのcanonical digestへ束縛され、独立受信時には明示documentから全要求を再導出する。実sourceのCode/InlineMathは構文のまま保持し、外部資源を読まず評価しない。これは準備入力の発見であり、下記の資源解決・HTML出力の完成ではない。

- 公開suiteでのhost guest-printの供給と、対象guest parserによる構造正規形・source対応の検査。現在の管理対象は実generic engine parser/checked tree/printerを使う構文往復であり、raw PrintedGuestのtextだけで一致proofを得ない。この残りは全guestの意味lower/checkを要求するものではなく、意味不正・回復構文を表示用Codeとして保持する契約を維持する。Math/Circuit等の意味check・評価の完成でもない。
- 外部 page・asset 解決、foreign Requirementを含む完全なcheck/prepare、HTML backend と rendering。schema に表・list・link・code・asset があることだけで、これらの実装済みを主張しない。
- Doc の正式 lower 操作の Report/部分結果包絡と全 suite adapter。native helper の Result を、別実装の操作包絡の完成として扱わない。

設計入力は main `b5295cef655aa59affffd6644f2902268d071953` の文書監査。inventory SHA-256 は `daf94085913930f05c1655d2adbef4f56651864f9a97ac4d261400449c98499e`、61 Markdown / 231 Rust source owner、52 表 / 1283 cell、39 list / 233 item、1134 inline code、12 code block、249 link、1 image。追加 element category はない。この監査は実装中差分の completeness、rustdoc 意味監査、T21 の移行完了とは別である。

## Tokenizer同期接続の次段階

prefix engineからtokenizerの同期hostを使用し、成功callで外側tokenizer継続を発行しない
経路を追加した。独立レビューで見つかった明示Stoppedエラーと入れ子の停止原因もBudgetへ
保持する。既存所有経路との構文木・診断・source/map比較を維持する。
HTMLの全文再現試験では同じWork上限100,000,000のまま停止位置が1412から1897へ進んだが、
13KB文書の完走には至っていない。これは部分改善であり、文書移行やHTML全体の成功ではない。
次は共有sourceの実コピー費用と外部echo比較費用の分離を検討し、外部入力の検査を維持して
本文の反復コピー課金を解消する。

続く実装では、実コピーと比較前の走査費用を分離し、source storeの重複検索を
予算付きの二分探索へ変更した。公開snapshot列の順序、外部echoの全文比較上限、
source編集の原子的な反映は維持する。同期reader callbackは外部継続を受け取らず、
排他的なprivate slotを既存の返却値検査経路へ渡す。
同じ全文試験は停止位置3399まで進んだが、なおWork上限で停止する。
途中の線形検索版ではGrammar bootstrapも既存上限で停止したため、その版を完成扱いにせず、
二分探索版でbootstrapを再実行して成功を確認した。workspace試験とClippyも成功した。
当時の残件はsource admission・診断source・Origin graphの繰り返し検索だった。

2026-09-13、`272228c`で同じ約13KB全文を`doc-html export`へ再入力したところ、
既存の各操作Work上限100,000,000のままHTMLまで成功した。使用Workはparse/validateが
20,362,601、lowerが2,987,790、prepare/render/serializeが6,986,518だった。
これらは別々の予算を持つ操作であり、一つのend-to-end予算の値ではない。
入力SHA-256は`9099bbbe48ed75a0768997789229188f53e43348fe930522b119908dfdb7a889`。
上記3399での停止は過去の途中版の記録で、現行の再現結果ではない。
この全文HTML試験はnative/WASIで成功している。多数ページ、全foreign、Pages配信の
性能・完成までを一例の成功から推定しない。

## Grammar仕様原稿の検査と割当課金

第04章の未公開NEPL3d原稿を、既存の原稿用parse/lower/labels試験へ追加した。
前段の修正時点では、通常ページのAllocation上限500,000,000で完成木の検査中に停止した。
その時点ではcanonical registryへ登録せず、Markdown正本を維持した。
未公開原稿用の既存の有限予算で処理できることと、正本切替の準備完了は区別する。

調査で、SyntaxBundle検査が共有SourceSnapshotにも本文コピー分を課金し、
借用するOrigin配列にも所有配列分を課金していたことが分かった。
SourceStoreの予算付き参照挿入を使い、Origin検査自身が計上するscratchだけを課金する。
Source入場、重複拒否、全Originの参照・cycle検査、非atomic targetの実コピー費用は維持する。
長短sourceの共有費用、source挿入時の停止、未使用Originの不正参照を回帰試験にした。
この課金修正のみでは第04章は通常上限で停止し、性能課題は未解消だった。

続く計測で、schema検査のpending stackがpop後の容量を再利用していても、
各pushで新規slot分を課金していたことが分かった。論理的な予約slot数を保持し、
倍増時の追加容量を確保前に課金する。allocatorの余剰capacityは課金判定に使わない。
各子のWorkはqueue投入前に課金し、広い入力の無制限な展開を防ぐ。
全値の型検査・Nodes・深さ・左から右の検査順は維持する。

この修正で第04章は通常予算内のparse/lower/labelsを通過した。
parseのWorkは94,666,857、Allocationは341,191,768、lowerのWorkは85,689,406だった。
SourceMapの一意な直接対応を範囲として検査する改善も加えたが、
この原稿の停止解消に寄与したのはschema stackの課金修正である。
一文書の成功を全仕様移行や全受入群の達成とは扱わない。

## Doc仕様原稿の次の性能境界

第05章の原稿で欠けていたguest printerの深さ合成とDoc/Math adapterの停止契約を補い、
独立Sentence readerから現Doc consumerへ変換する段階を正文と原稿に明記した。
修正後のCLIによる通常Work上限100,000,000でのHTML出力はcursor 47,445でWorkLimitとなり、
正本切替は行っていない。

Work計測からSourceMapのsnapshot検索を調べ、直前の検索位置に隣接するidentityを
二分探索の前に照合する改善を加えた。完全なSnapshotIdの比較、cycle検査、重複edge、
各比較前の課金を維持する。順序付きedgeを反復する257頂点のstarでは、修正前の
WorkLimitに対し修正後は435,257 Workで検査できた。第05章は通常予算で停止しており、
この小さい回帰試験を同原稿の性能課題の解消とは扱わない。

続いてreaderのprovider境界で、新規mappingがなく全包含関係を同一snapshot内で
直接確認できる場合に限り、private checkpointで検査済みのmapping集合の再検査を省いた。
viewのschema・参照・cycle、factのmetadataとsource位置の検査は引き続き実行する。
間接的な包含関係または新規mappingがある場合は従来通り集合全体を検査する。
不正KindRef、空Capture名、未登録Presentation/Relation schema、追加mapによるcycle、
停止予算を負例として検査した。readerのnative試験とWASI runtime試験、
追加負例を含むlibrary試験は成功し、独立レビューの指摘を修正した。

同じ第05章の通常HTML出力はcursor 54,300まで進んだが、Work上限100,000,000で
停止した。予算を変更しておらず、第05章の正本切替および性能課題の解消は未達である。

次のWork計測ではtokenizerの受入済みsourceに対する競合検索が約30,804,689を占めた。
単発検索のhintとbatch検索は実文書で改善せず撤回した。
tokenizer session内で、照合した宣言集合と受入済みprefixのimmutable snapshotを保持し、
次回も完全一致する部分の競合検索だけを再利用する。宣言集合の変更・prefix変更は
再検査し、source admission・report検査・継続scope検査は従来通り実行する。
保持するsnapshotは所有し、allocatorのaddress再利用を同一性の根拠にしない。
外部decode値は内容比較を省かず、closeで保持情報を解放する。
検査の順序・Usageは変わり得るが、停止を成功へ変換したり新しい予算へ移したりしない。

この変更の通常HTML出力はcursor 70,193でWorkLimitとなった。上限は同じ100,000,000で、
第05章の完走と正本化はまだ未達である。環境変更、変更されたsuffix、同一内容の
別storage、部分的なcache確保後の停止と再検査を回帰試験で確認する。

さらに、新しいsource/mapと診断/eventがなく、全artifactの位置を消費範囲内で
直接確認できるprovider応答では、そのsnapshotだけで通常の応答検査を行う。
範囲外の合法なPresentation/Relationは従来のresolverへ戻し、不正なschema・参照・
report、新規sourceの競合を省略しない。private checkpointの無関係なsource集合を
tokenごとに再構築していた費用を減らした。

第05章は通常予算でparse 88,653,715 Work、lower 66,285,115 Work、
labels 8,413,256 Workに収まり、この予算を原稿の回帰試験にも適用した。
単独HTML出力はWorkLimitではなく17章・23章への2リンクのNeedsResolutionまで進む。
これはHTML生成・リンク解決・文書の意味比較・正本切替の完了を意味しない。

第05章の正本移行では、原稿を `doc/spec/05-document.nepld` へ移し、旧見出し16件を
aliasとして登録した。第23章は未移行Markdownの原文参照であり、Doc正本へ昇格させない。
本文の独立比較でinline code92個、RawCode2個、15節、リンク2件の保存を確認した。
18ページのHTML集合は通常のpage予算で生成でき、第05章のparseは87,582,341 Work、
lowerは65,610,740 Workとなった。JavaScript無効のChromium/Firefox/WebKitで本文と
17章HTML・23章原文へのリンクを確認した。混在siteでは23章を既生成HTMLへ接続する。

Markdown集合の旧Work上限1,600,000,000では停止し、途中出力を採用しなかった。
18ページ用に集合Workだけを1,900,000,000へ変更した別実行は1,615,489,130 Work、
63,205,674 Nodesで完了した。Nodes上限64,000,000と各pageの通常予算は維持する。
既存17ページの本文は不変で、集合入力digestに伴うmetadataだけが更新される。
停止診断にはpage・段階・Usageを加え、文書集合の停止と単体解析の停止を区別する。
この移行はT07/T21全体や未移行の残り6章の完成を意味しない。
