import { CodeAction, WorkspaceEdit, Range as ClientRange, Position, TextDocument } from "vscode";

import { CodeActionParams, LanguageClient, LanguageClientOptions, Range } from "vscode-languageclient/node";
export type CodeActionHandler = Parameters<LanguageClientOptions["middleware"]["provideCodeActions"]>;
export type ActionHandlerReturnType = ReturnType<LanguageClientOptions["middleware"]["provideCodeActions"]>;

export const getCodeActionFromServer: (...args: [LanguageClient, ...Partial<CodeActionHandler>]) => Promise<any> = (
  client,
  doc,
  range,
  context,
  token
) => {
  const params: CodeActionParams = {
    textDocument: client.code2ProtocolConverter.asTextDocumentIdentifier(doc),
    range: client.code2ProtocolConverter.asRange(range),
    context: client.code2ProtocolConverter.asCodeActionContext(context),
  };
  return client
    .sendRequest("textDocument/codeAction", params, token)
    .then(res => res || [])
    .catch(err => {
      return [];
    });
};

export const codeActionProvider: (...args: [LanguageClient, ...Partial<CodeActionHandler>]) => ActionHandlerReturnType =
  async (client, doc, range, context, token) => {
    // return getCodeActionFromServer(doc,range, context, token );
    let result = [];
    try {
      let res = await Promise.race([
        getCodeActionFromServer(client, doc, range, context, token),
        new Promise((resolve, reject) => {
          setTimeout(() => {
            resolve([]);
          }, 1000);
        }),
      ]);
      // debugger;
      result = (res || []).map(item => {
        const normalizedItem = client.protocol2CodeConverter.asCodeAction(item);
        if (normalizedItem.title === "extract react component") {
          convertExtractComponentAction(normalizedItem, doc);
        }
        return normalizedItem;
      });
    } catch (err) {
      console.error(err);
    }
    if (!range.isEmpty) {
      const content = doc.getText(range);
      const action: CodeAction = {
        title: "insert bench label",
        command: {
          title: "insert-bench-label",
          command: "tjs-postfix.insert-bench-label",
          arguments: [doc.uri, range, content],
        },
      };
      result.push(action);
    }
    return result;
  };

interface IdentifierNode {
  start: number;
  end: number;
  range: Range;
  name: string;
}

interface ExtractComponentData {
  identifierNodeList: IdentifierNode[];
  jsxElementRange: Range;
}

function convertExtractComponentAction(normalizedItem: CodeAction, doc: TextDocument) {
  const data: ExtractComponentData = (normalizedItem as any).data;
  if (!data) {
    return normalizedItem;
  }
  const {
    identifierNodeList,
    jsxElementRange: { end, start },
  } = data;
  const normalizedJsxElementRange = new ClientRange(
    new Position(start.line, start.character),
    new Position(end.line, end.character)
  );
  let edit = new WorkspaceEdit();
  let docLength = doc.getText().length;
  let endPosition = doc.positionAt(docLength);
  let jsxElementText = doc.getText(normalizedJsxElementRange);
  let componentFunction = "";
  // if (doc.languageId === "javascriptreact" || doc.languageId === "javascript") {
  componentFunction = `
function Component1({${identifierNodeList.map(item => item.name).join(",")}}) {
  return ${jsxElementText}
} 
`;
  normalizedItem.title += ` ${componentFunction}`;
  let componentInvoke = `<Component1 ${identifierNodeList.map(item => `${item.name}={${item.name}}`).join(" ")}/>`;
  edit.insert(doc.uri, endPosition, componentFunction);
  edit.replace(doc.uri, normalizedJsxElementRange, componentInvoke);
  normalizedItem.edit = edit;
  normalizedItem.command = {
    command: "tjs-postfix.move-cursor",
    title: "cursorMove",
    arguments: [
      {
        start: normalizedJsxElementRange.start,
        end: normalizedJsxElementRange.start,
      },
    ],
  };
}
