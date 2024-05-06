import * as vscode from 'vscode';
import { createDecorationType } from './decorations/helpers';
import { Logger } from './logger';

export class Decorations {
    wasm: typeof import("typst-math-rust") | undefined;
    allDecorations: {
        [key: string]: {
            decorationType: vscode.TextEditorDecorationType,
            ranges: vscode.DecorationOptions[],
        }
    } = {};
    selection_timeout: NodeJS.Timeout | undefined = undefined;
    last_selection_line = { start: -1, end: -1 };
    editing = false;
    activeEditor = vscode.window.activeTextEditor;

    // Init the WASM lib
    async init() {
        this.wasm = await import("typst-math-rust");
        this.wasm.init_lib();
    }

    // Render decorations, while revealing current line
    renderDecorations() {
        console.time("renderDecorations");
        if (this.activeEditor?.selection) {
            let selection = this.activeEditor.selection;
            let reveal_selection = new vscode.Range(new vscode.Position(selection.start.line, 0), new vscode.Position(selection.end.line + 1, 0));

            for (let t in this.allDecorations) {
                this.activeEditor?.setDecorations(
                    this.allDecorations[t].decorationType,
                    this.allDecorations[t].ranges.filter(range => {
                        return range.range.intersection(reveal_selection) === undefined;
                    })
                );
            }
        } else {
            for (let t in this.allDecorations) {
                this.activeEditor?.setDecorations(
                    this.allDecorations[t].decorationType,
                    this.allDecorations[t].ranges
                );
            }
        }
        console.timeEnd("renderDecorations");
    }
    // Pass the current doc to typst to get symbols, and then render them
    reloadDecorations() {
        if (this.activeEditor && this.wasm) {
            console.time("reloadDecorations");
            for (let t in this.allDecorations) {
                this.allDecorations[t].ranges = [];
            }
            let test = this.activeEditor;
            let decorations = this.wasm.parse_document(this.activeEditor.document.getText() as string);
            for (let decoration of decorations) {
                if (!this.allDecorations.hasOwnProperty(decoration.content)) {
                    this.allDecorations[decoration.content] = {
                        decorationType: createDecorationType({
                            contentText: decoration.symbol.symbol
                        }),
                        ranges: []
                    };
                }
                let ranges = decoration.positions.map<vscode.DecorationOptions>((pos) => {
                    return {
                        range: new vscode.Range(test.document.positionAt(pos.start), test.document.positionAt(pos.end)),
                    };
                });
                this.allDecorations[decoration.content].ranges = ranges;
            }
            console.timeEnd("reloadDecorations");
            this.renderDecorations();
            Logger.info(`Loaded ${decorations.length} decorations`);
        }
    }

    // When the selection change, check if a reload and/or a render is needed
    onSelectionChange(event: vscode.TextEditorSelectionChangeEvent) {
        if (this.activeEditor && event.textEditor === this.activeEditor) {
            if (this.last_selection_line.start !== event.selections[0].start.line || this.last_selection_line.end !== event.selections[0].end.line) { // The cursor changes of line
                this.last_selection_line.start = event.selections[0].start.line;
                this.last_selection_line.end = event.selections[0].start.line;

                // If the selection changes, update the decorations after a short delay, to avoid updating the decorations too often
                if (this.selection_timeout) {
                    clearTimeout(this.selection_timeout);
                }

                if (this.editing) { // Text was typed, reload completely decorations
                    this.editing = false;
                    this.selection_timeout = setTimeout(async () => {
                        this.reloadDecorations();
                    }, 200);
                } else { // Only cursor was moved, just render decorations by revealing current line
                    this.selection_timeout = setTimeout(async () => {
                        this.renderDecorations();
                    }, 50); // 50ms to keep things fast, but not to quick to avoid rendering too often
                }
            }
        }
    }

    // This event is useful for finding out, when selection changes, if the last changes were made by typing or simply moving the cursor
    onTextDocumentChange(event: vscode.TextDocumentChangeEvent) {
        if (this.activeEditor && event.document === this.activeEditor.document) {
            if (event.contentChanges.length === 0) { return; }
            this.editing = true;
        }
    }

    // When the editor change, update activeEditor and reload decorations
    onActiveTextEditorChange(editor: vscode.TextEditor | undefined) {
        this.activeEditor = editor;
        if (this.activeEditor) {
            this.reloadDecorations();
        }
    }
}