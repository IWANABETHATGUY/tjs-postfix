import {
  ExtensionContext,
  languages,
  TextDocument,
  CompletionItemProvider,
  Position,
  CancellationToken,
  CompletionList,
  CompletionItem,
  CompletionContext,
  CompletionItemKind,
  Disposable,
  TextEdit,
  Range,
  workspace,
} from "vscode";

import * as path from "path";
import Parser from "tree-sitter";
import Typescript from "tree-sitter-typescript/typescript";

export class VueTemplateCompletion {
  private _context: ExtensionContext;
  _completion!: TemplateCompletion;
  constructor(context: ExtensionContext) {
    this._context = context;
    this.init();
  }

  private init(): void {
    this.initCompletion();
  }

  private resetComponentMetaData(): void {}

  private initCompletion(): void {
    this._completion = new TemplateCompletion();
    this._context.subscriptions.push(
      languages.registerCompletionItemProvider(
        [
          { language: "typescript", scheme: "file" },
          { language: "javascript", scheme: "file" },
          { language: "vue", scheme: "file" },
        ],
        this._completion,
        "."
      )
    );
  }
}

type CompletionMap = {
  event: CompletionItem[];
  prop: CompletionItem[];
  slot: CompletionItem[];
};
type TemplateMap = {
  snippetKey: string;
  functionName: string;
};
export class TemplateCompletion implements CompletionItemProvider {
  private _disposable: Disposable;
  parser: Parser;
  tree!: Parser.Tree;
  templateList!: TemplateMap[];
  constructor() {
    const subscriptions: Disposable[] = [];
    this.parser = new Parser();
    this.parser.setLanguage(Typescript);
    this._disposable = Disposable.from(...subscriptions);
    this.initTemplateList();
    workspace.onDidChangeConfiguration(e => {
      if (e.affectsConfiguration("tjs-postfix.templateMapList")) {
        this.initTemplateList();
      }
    });
  }
  initTemplateList() {
    this.templateList = workspace.getConfiguration("tjs-postfix")?.get("templateMapList") ?? [];
    this.templateList = this.templateList.filter(item => {
      return item.functionName && item.snippetKey;
    });
  }
  dispose(): void {
    this._disposable.dispose();
  }

  async provideCompletionItems(
    document: TextDocument,
    position: Position,
    token: CancellationToken,
    context: CompletionContext
  ): Promise<CompletionItem[] | CompletionList> {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    // let curNode = curTree.rootNode.namedDescendantForPosition({
    //   column: position.character,
    //   row: position.line,
    // });
    const range = document.getWordRangeAtPosition(position, /[^\s]\.[a-zA-Z]*/);
    if (!range) {
      return [];
    }

    const beforeDot = range.start;
    const doc = document.getText();
    this.tree = this.parser.parse(doc);

    let curNode = this.tree.rootNode.namedDescendantForPosition({
      column: beforeDot.character,
      row: beforeDot.line,
    });
    let endIndex = curNode.endIndex;
    while (true) {
      if (curNode.parent && curNode.parent.endIndex === endIndex && curNode.type !== "ERROR") {
        curNode = curNode.parent;
      } else {
        break;
      }
    }
    // console.log(curNode.type);
    return this.templateList.map(template => {
      const item = new CompletionItem(template.snippetKey);
      item.kind = CompletionItemKind.Snippet;
      item.insertText = "";
      item.keepWhitespace = true;
      const replaceString = `${template.functionName}(${curNode.text})`;
      item.documentation = replaceString;
      const replaceRange = new Range(
        curNode.startPosition.row,
        curNode.startPosition.column,
        range.end.line,
        range.end.character
      );
      // console.log(curNode.text);
      item.additionalTextEdits = [TextEdit.replace(replaceRange, replaceString)];
      return item;
    });
  }
}
