// @ts-ignore Typescript doesn't see the use directive
import { copyToClipboard } from "@solid-primitives/clipboard";
import { listen } from "@tauri-apps/api/event";
import { createResource, createSignal, For, Match, Switch } from "solid-js";
import toast, { Toaster } from "solid-toast";

type State = "disabled" | "detecting" | "enabled";

function App() {
    const [ocrStrings, setOcrStrings] = createSignal<string[]>([]);
    const [state, setState] = createSignal<State>("disabled");
    createResource(
        async () =>
            await listen("state-changed", (event) => {
                console.log(event);
                setState(event.payload as State);
            })
    );
    createResource(
        async () =>
            await listen("ocr-changed", (event) => {
                console.log(event);
                setOcrStrings(event.payload as string[]);
            })
    );

    return (
        <div class="p-4 w-full h-full">
            <Switch>
                <Match when={state() == "enabled"}>
                    <h1 class="text-xl leading-loose text-center font-semibold">
                        Detected Strings
                    </h1>
                    <div class="flex-col divide-y divide-slate-600 border border-slate-300">
                        <For each={ocrStrings()}>
                            {(text, _) => (
                                <p
                                    class="text-center py-2 cursor-pointer"
                                    title="Copy to clipboard"
                                    onClick={(_) =>
                                        toast("Copied to clipboard")
                                    }
                                    use:copyToClipboard
                                >
                                    {text}
                                </p>
                            )}
                        </For>
                    </div>
                </Match>
                <Match when={state() == "detecting"}>
                    <h1 class="text-xl leading-loose text-center">
                        Detecting...
                    </h1>
                    <div class="loader"></div>
                </Match>
                <Match when={state() == "disabled"}>
                    <h1 class="text-xl leading-loose text-center">Disabled</h1>
                    <p class="text-sm text-slate-300 text-center">
                        Press Alt+X to toggle
                    </p>
                </Match>
            </Switch>
            <Toaster />
        </div>
    );
}

export default App;
