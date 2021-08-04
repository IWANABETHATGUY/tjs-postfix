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
} from "vscode";

import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient;

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
    middleware: {
      provideCodeActions(doc, range, context, token) {
        return new Promise((resolve, reject) => {
          const result = [];
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
          resolve(result);
        });
      },
    },
    traceOutputChannel,
  };

  // Create the language client and start the client.
  client = new LanguageClient("tjs-postfix", "TJS Language Server", serverOptions, clientOptions);
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
