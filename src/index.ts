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

type ComponentCompletionMap = {
  [componentName: string]: CompletionMap;
};

enum MyCompletionPositionKind {
  StartTag,
  DirectiveAttribute,
  Attribute,
}

const directiveAttributeRegExp = /[\w_@\-\:]+/;

// HACK: 目前的tagName 转化以及对比做的不好，需要优化

export class TemplateCompletion implements CompletionItemProvider {
  private _disposable: Disposable;
  parser: Parser;
  tree!: Parser.Tree;
  constructor() {
    const subscriptions: Disposable[] = [];
    this.parser = new Parser();
    this.parser.setLanguage(Typescript);
    this._disposable = Disposable.from(...subscriptions);
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
    console.time("start");
    this.tree = this.parser.parse(doc);
    // tree.rootNode.descendantForPosition(position);
    // debugger;
    let curNode = this.tree.rootNode.namedDescendantForPosition({
      column: beforeDot.character,
      row: beforeDot.line,
    });
    console.timeEnd("start");
    // curNode.namedDescendantForPosition(position);
    let endIndex = curNode.endIndex;
    while (true) {
      if (curNode.parent && curNode.parent.endIndex === endIndex && curNode.type !== "ERROR") {
        curNode = curNode.parent;
      } else {
        break;
      }
    }
    console.log(curNode.type);
    const item = new CompletionItem("log");
    item.insertText = "";
    
    const edit = new TextEdit(
      new Range(curNode.startPosition.row, curNode.startPosition.column, range.end.line, range.end.character),
      `console.log(${curNode.text})`
    );
    console.log(curNode.text);
    item.additionalTextEdits = [edit];
    return [item];
  }

  // [39, 43].includes(curNode.parent.typeId)
}
