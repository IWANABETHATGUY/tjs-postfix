import {
	CodeAction,
	WorkspaceEdit,
	Range as ClientRange,
	Position,
	TextDocument,
} from "vscode";

import {
	CodeActionParams,
	LanguageClient,
	LanguageClientOptions,
	Range,
} from "vscode-languageclient/node";
export type CodeActionHandler = Parameters<
	LanguageClientOptions["middleware"]["provideCodeActions"]
>;
export type ActionHandlerReturnType = ReturnType<
	LanguageClientOptions["middleware"]["provideCodeActions"]
>;

export const getCodeActionFromServer: (
	...args: [{ tjsc: LanguageClient }, ...Partial<CodeActionHandler>]
) => Promise<any> = ({ tjsc }, doc, range, context, token) => {
	const params: CodeActionParams = {
		textDocument: tjsc.code2ProtocolConverter.asTextDocumentIdentifier(doc),
		range: tjsc.code2ProtocolConverter.asRange(range),
		context: tjsc.code2ProtocolConverter.asCodeActionContext(context),
	};
	return tjsc
		.sendRequest("textDocument/codeAction", params, token)
		.then((res) => res || [])
		.catch((err) => {
			return [];
		});
};

export const codeActionProvider: (
	...args: [{ tjsc: LanguageClient }, ...Partial<CodeActionHandler>]
) => ActionHandlerReturnType = async ({ tjsc }, doc, range, context, token) => {
	if (range.isSingleLine && (range.end.character - range.start.character) < 3) {
		return null;
	}
	// return getCodeActionFromServer(doc,range, context, token );
	let result = [];
	try {
		let res = await Promise.race([
			getCodeActionFromServer({ tjsc }, doc, range, context, token),
			new Promise((resolve, reject) => {
				setTimeout(() => {
					resolve([]);
				}, 1000);
			}),
		]);
		res = res || [];
		result.length = res.length;
		for (let i = 0; i < res.length; i++) {
			let item = res[i];
			const normalizedItem = tjsc.protocol2CodeConverter.asCodeAction(item);
			if (normalizedItem.title === "extract react component") {
				try {
					await convertExtractComponentAction(normalizedItem, doc);
				} catch {}
			}
			result.push(normalizedItem);
		}
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

export interface ExtractComponentData {
	identifierNodeList: IdentifierNode[];
	jsxElementRange: Range;
	functionName: string;
	path?: string;
	jsxElementText?: string;
	endPosition: Position;
}

async function convertExtractComponentAction(
	normalizedItem: CodeAction,
	doc: TextDocument,
) {
	const data: ExtractComponentData = (normalizedItem as any).data;
	if (!data) {
		return normalizedItem;
	}
	const {
		identifierNodeList,
		jsxElementRange: { end, start },
		functionName,
	} = data;
	data.path = doc.uri.fsPath;
	const normalizedJsxElementRange = new ClientRange(
		new Position(start.line, start.character),
		new Position(end.line, end.character),
	);
	let edit = new WorkspaceEdit();
	let docLength = doc.getText().length;
	let endPosition = doc.positionAt(docLength);
	let jsxElementText = doc.getText(normalizedJsxElementRange);
	if (doc.languageId === "javascript" || doc.languageId === "javascriptreact") {
		let componentFunction = `
function ${functionName}({${generateParametersForJsx(identifierNodeList)}}) {
  return ${jsxElementText}
} 
`;
		edit.insert(doc.uri, endPosition, componentFunction);
	} else {
		// let typeList = await getTypeFromTypescriptService(
		//   tsc,
		//   doc.uri.fsPath,
		//   identifierNodeList.map(item => item.start)
		// );
		let idList = identifierNodeList.map((item) => item.name);
		let typeList = identifierNodeList.map((item) => "any");

		let componentFunction = `
    function ${functionName}(${generateParametersForTsx(
			identifierNodeList,
			typeList,
			idList,
		)}) {
      return ${jsxElementText}
    }
    `;
		edit.insert(doc.uri, endPosition, componentFunction);
	}
	let componentInvoke = `<${functionName} ${identifierNodeList.map(
		(item) => `${item.name}={${item.name}}`,
	).join(" ")}/>`;
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

function generateParametersForJsx(identifierNodeList: IdentifierNode[]) {
	return identifierNodeList.length ? identifierNodeList.map(
		(item) => item.name,
	).join(",") : "";
}

function getTypeFromTypescriptService(
	tsc: LanguageClient,
	path: string,
	posList: number[],
): Promise<string[]> {
	return new Promise((resolve, reject) => {
		tsc.sendRequest("test", {
			path: path,
			posList: posList,
		});
		tsc.onRequest("response-test", async (res) => {
			resolve(res);
		});
		setTimeout(reject, 500);
	});
}

function generateTypeOfComponentParams(typeList: string[], idList: string[]) {
	let typeInner = "";
	for (let i = 0; i < typeList.length; i++) {
		typeInner += `${idList[i]}:${typeList[i] || "any"},`;
	}
	return `{${typeInner}}`;
}

function generateParametersForTsx(
	identifierNodeList: IdentifierNode[],
	typeList: string[],
	idList: string[],
) {
	return identifierNodeList.length ? `{${identifierNodeList.map(
		(item) => item.name,
	).join(",")}}: ${generateTypeOfComponentParams(typeList, idList)}` : "";
}
