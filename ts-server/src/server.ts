/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */
import {
  createConnection,
  TextDocuments,
  ProposedFeatures,
  InitializeParams,
  DidChangeConfigurationNotification,
  TextDocumentSyncKind,
  InitializeResult,
} from "vscode-languageserver/node";
import * as path from "path";
import * as glob from "glob";
import { URI } from "vscode-uri";
import { Position, TextDocument } from "vscode-languageserver-textdocument";
import { promisify } from "util";
import { watch } from "./typescript_server";
import * as ts from "typescript";
// Create a connection for the server, using Node's IPC as a transport.
// Also include all preview / proposed LSP features.
const connection = createConnection(ProposedFeatures.all);
const asyncGlob = promisify(glob);
// Create a simple text document manager.
const documents: TextDocuments<TextDocument> = new TextDocuments(TextDocument);

let hasConfigurationCapability = false;
let hasWorkspaceFolderCapability = false;
let hasDiagnosticRelatedInformationCapability = false;
let languageService: ReturnType<typeof watch>;
connection.onInitialize((params: InitializeParams) => {
  const capabilities = params.capabilities;
  // Does the client support the `workspace/configuration` request?
  // If not, we fall back using global settings.
  const workspace = params.workspaceFolders?.[0];
  if (workspace) {
    const fsPath = URI.parse(workspace.uri).fsPath;
    connection.console.log(path.join(fsPath, "/**/*.ts"));
    let fileList: string[] = [];
    const p1 = asyncGlob(path.join(fsPath, "/**/*.ts"), { ignore: "**/node_modules/**" }).then(res => {
      fileList = fileList.concat(res);
    });
    const p2 = asyncGlob(path.join(fsPath, "/**/*.tsx"), { ignore: "**/node_modules/**" }).then(res => {
      fileList = fileList.concat(res);
    });
    Promise.all([p1, p2]).then(() => {
      if (fileList.length) {
        console.time("createLanguageService");
        languageService = watch(fileList, {});
        const program = languageService.getProgram();
        if (program) {
          console.timeEnd("createLanguageService");
        }
      }
    });
  }
  hasConfigurationCapability = !!(capabilities.workspace && !!capabilities.workspace.configuration);
  hasWorkspaceFolderCapability = !!(capabilities.workspace && !!capabilities.workspace.workspaceFolders);
  hasDiagnosticRelatedInformationCapability = !!(
    capabilities.textDocument &&
    capabilities.textDocument.publishDiagnostics &&
    capabilities.textDocument.publishDiagnostics.relatedInformation
  );

  const result: InitializeResult = {
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Incremental,
      // Tell the client that this server supports code completion.
    },
  };
  if (hasWorkspaceFolderCapability) {
    result.capabilities.workspace = {
      workspaceFolders: {
        supported: true,
      },
    };
  }
  return result;
});

connection.onInitialized(() => {
  if (hasConfigurationCapability) {
    // Register for all configuration changes.
    connection.client.register(DidChangeConfigurationNotification.type, undefined);
  }
  if (hasWorkspaceFolderCapability) {
    connection.workspace.onDidChangeWorkspaceFolders(_event => {
      connection.console.log("Workspace folder change event received.");
    });
  }
});

// The example settings
interface ExampleSettings {
  maxNumberOfProblems: number;
}

// The global settings, used when the `workspace/configuration` request is not supported by the client.
// Please note that this is not the case when using this server with the client provided in this example
// but could happen with other clients.
const defaultSettings: ExampleSettings = { maxNumberOfProblems: 1000 };
let globalSettings: ExampleSettings = defaultSettings;

// Cache the settings of all open documents
const documentSettings: Map<string, Thenable<ExampleSettings>> = new Map();

connection.onDidChangeConfiguration(change => {
  if (hasConfigurationCapability) {
    // Reset all cached document settings
    documentSettings.clear();
  } else {
    globalSettings = <ExampleSettings>(change.settings.languageServerExample || defaultSettings);
  }

  // Revalidate all open text documents
});

function getDocumentSettings(resource: string): Thenable<ExampleSettings> {
  if (!hasConfigurationCapability) {
    return Promise.resolve(globalSettings);
  }
  let result = documentSettings.get(resource);
  if (!result) {
    result = connection.workspace.getConfiguration({
      scopeUri: resource,
      section: "languageServerExample",
    });
    documentSettings.set(resource, result);
  }
  return result;
}

// Only keep settings for open documents
documents.onDidClose(e => {
  documentSettings.delete(e.document.uri);
});

// The content of a text document has changed. This event is emitted
// when the text document first opened or when its content has changed.

connection.onDidChangeWatchedFiles(_change => {
  // Monitored files have change in VSCode
  connection.console.log("We received an file change event");
});
interface RequestParams {
  path: string;
  posList: number[];
}
connection.onRequest("test", async (params: RequestParams) => {
  try {
    if (!languageService) {
      return null;
    }
    const program = languageService.getProgram();
    if (!program) {
      return null;
    }
    const fileName = params.path;
    const typeList = params.posList.map(item => "");
    const sourceFile = program.getSourceFile(fileName);
    if (!sourceFile) {
      return null;
    }
    visit(sourceFile, params.posList, typeList, program.getTypeChecker());
    connection.client.connection.sendRequest("response-test", typeList);
  } catch {
    connection.client.connection.sendRequest("response-test", []);
  }
});
// This handler provides the initial list of the completion items.
// Make the text document manager listen on the connection
// for open, change and close text document events

documents.listen(connection);

// Listen on the connection
connection.listen();

function visit(node: ts.Node, posList: number[], typeList: string[], checker: ts.TypeChecker) {
  // console.log(node.kind);
  if (node.kind === ts.SyntaxKind.Identifier) {
    // let type = checker.getTypeAtLocation(node)
    const start = node.getStart();
    const end = node.getEnd();
    for (let i = 0; i < posList.length; i++) {
      const pos = posList[i];
      if (start <= pos && end >= pos) {
        const type = checker.getTypeAtLocation(node);
        const typeString = checker.typeToString(type, node, ts.TypeFormatFlags.InTypeAlias);
        typeList[i] = typeString;
        break;
      }
    }
  }
  const children = node.getChildren();
  for (let i = 0; i < children.length; i++) {
    const node = children[i];
    visit(node, posList, typeList, checker);
  }
}
