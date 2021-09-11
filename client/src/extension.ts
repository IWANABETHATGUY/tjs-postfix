/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

import * as path from "path";
import {
  workspace,
  ExtensionContext,
  window,
  commands,
  ViewColumn,
  WebviewPanel,
  CodeAction,
  WorkspaceEdit,
  Range as ClientRange,
  Position,
  TextDocument,
} from "vscode";

import {
  CodeActionParams,
  Executable,
  LanguageClient,
  LanguageClientOptions,
  Range,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient;
type CodeActionHandler = Parameters<LanguageClientOptions["middleware"]["provideCodeActions"]>;
type ActionHandlerReturnType = ReturnType<LanguageClientOptions["middleware"]["provideCodeActions"]>;
// type a = Parameters<>;
const getCodeActionFromServer: (...args: Partial<CodeActionHandler>) => Promise<any> = (doc, range, context, token) => {
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
export async function activate(context: ExtensionContext) {
  // The server is implemented in node
  // let serverModule = context.asAbsolutePath(
  // 	path.join('server', 'out', 'server.js')
  // );
  // The debug options for the server
  // --inspect=6009: runs the server in Node's Inspector mode so VS Code can attach to the server for debugging
  // let debugOptions = { execArgv: ['--nolazy', '--inspect=6009'] };

  // E:\vscode-extension\github\server\target\debug
  let currentPanel: WebviewPanel | undefined = undefined;
  context.subscriptions.push(
    commands.registerCommand("tjs-postfix.ast-preview", () => {
      const columnToShowIn = window.activeTextEditor ? window.activeTextEditor.viewColumn : undefined;

      if (currentPanel) {
        // If we already have a panel, show it in the target column
        currentPanel.reveal(columnToShowIn);
      } else {
        // Create and show a new webview
        currentPanel = window.createWebviewPanel(
          "tjs-postfix.ast-preview", // Identifies the type of the webview. Used internally
          "ast-preview", // Title of the panel displayed to the user
          ViewColumn.Two, // Editor column to show the new webview panel in.
          {} // Webview options. More on these later.
        );
        client.sendRequest("tjs-postfix/ast-preview", {
          path: window.activeTextEditor.document.uri.toString(),
        });
      }

      currentPanel.onDidDispose(
        () => {
          currentPanel = undefined;
        },
        null,
        context.subscriptions
      );
    })
  );
  context.subscriptions.push(
    commands.registerCommand("tjs-postfix.insert-bench-label", async (uri, range, content) => {
      try {
        const result = await window.showInputBox({
          value: "label",
          placeHolder: "input your bench label",
        });
        const edit = new WorkspaceEdit();
        edit.replace(
          uri,
          range,
          `console.time('${result}')
${content}
console.timeEnd('${result}')`
        );
        workspace.applyEdit(edit);
      } catch (e) {
        console.warn(e);
      }
    })
  );
  const traceOutputChannel = window.createOutputChannel("Tjs language server trace");
  const command = process.env.SERVER_PATH || "tjs-language-server";
  const run: Executable = {
    command,
    options: {
      env: {
        ...process.env,
        // eslint-disable-next-line @typescript-eslint/naming-convention
        RUST_LOG: "debug",
      },
    },
  };
  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };
  // If the extension is launched in debug mode then the debug server options are used
  // Otherwise the run options are used

  // Options to control the language client
  let clientOptions: LanguageClientOptions = {
    // Register the server for plain text documents
    documentSelector: [
      { scheme: "file", language: "typescript" },
      { scheme: "file", language: "javascript" },
      { scheme: "file", language: "vue" },
      { scheme: "file", language: "typescriptreact" },
      { scheme: "file", language: "javascriptreact" },
    ],
    synchronize: {
      // Notify the server about file changes to '.clientrc files contained in the workspace
      fileEvents: workspace.createFileSystemWatcher("**/.clientrc"),
    },
    middleware: {},
    traceOutputChannel,
  };

  // Create the language client and start the client.
  client = new LanguageClient("tjs-postfix", "TJS Language Server", serverOptions, clientOptions);

  context.subscriptions.push(
    commands.registerCommand("tjs-postfix.restart-language-server", async (uri, range, content) => {
      try {
        client.stop();
        client.start();
      } catch (e) {
        console.error(e);
      }
    })
  );

  client.clientOptions.middleware.provideCodeActions = codeActionProvider;

  // Create the language client and start the client.
  // Start the client. This will also launch the server
  client.onReady().then(() => {
    client.onNotification("tjs-postfix/notification", (...args) => {
      console.log(...args);
      if (args[0] && currentPanel) {
        const { message: astString, title: path } = args[0];
        currentPanel.webview.html = getWebContent(path, astString);
      }
    });
  });

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

function getWebContent(path: string, astString: string): string {
  return `
    <h2>${path}</h2>
    <pre>${astString}</pre>
  `;
}

const codeActionProvider: (...args: Partial<CodeActionHandler>) => ActionHandlerReturnType = async (
  doc,
  range,
  context,
  token
) => {
  // return getCodeActionFromServer(doc,range, context, token );
  let result = [];
  try {
    let res = await Promise.race([
      getCodeActionFromServer(doc, range, context, token),
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
  let componentFunction = `
function Component1({${identifierNodeList.map(item => item.name).join(",")}}) {
  return ${jsxElementText}
} 
`;
  let componentInvoke = `<Component1 ${identifierNodeList.map(item => `${item.name}={${item.name}}`).join(" ")}/>`;
  edit.insert(doc.uri, endPosition, componentFunction);
  edit.replace(doc.uri, normalizedJsxElementRange, componentInvoke);
  normalizedItem.edit = edit;
  // result.edit.insert(doc.uri, doc, newText)
}
