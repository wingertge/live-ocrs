import { createElementSize } from "@solid-primitives/resize-observer";
import { invoke } from "@tauri-apps/api";
import { listen } from "@tauri-apps/api/event";
import { createEffect, createResource, createSignal, For } from "solid-js";

type Definition = {
    simplified: string;
    traditional: string;
    pinyin: { tone: number; syllable: string }[];
    translations: string[];
};

function App() {
    const [definitions, setDefinitions] = createSignal<Definition[]>([
        {
            simplified: "早上好",
            traditional: "早上好",
            pinyin: [
                {
                    tone: 3,
                    syllable: "zǎo",
                },
                {
                    tone: 5,
                    syllable: "shang",
                },
                {
                    tone: 3,
                    syllable: "hǎo",
                },
            ],
            translations: ["Good morning!"],
        },
        {
            simplified: "早上",
            traditional: "早上",
            pinyin: [
                {
                    tone: 3,
                    syllable: "zǎo",
                },
                {
                    tone: 5,
                    syllable: "shang",
                },
            ],
            translations: ["early morning", "CL:個|个[ge4]"],
        },
        {
            simplified: "早",
            traditional: "早",
            pinyin: [
                {
                    tone: 3,
                    syllable: "zǎo",
                },
            ],
            translations: [
                "early",
                "morning",
                "Good morning!",
                "long ago",
                "prematurely",
            ],
        },
    ]);
    createResource(
        async () =>
            await listen("definitions-changed", (event) => {
                //console.log(event.payload);
                setDefinitions(event.payload as Definition[]);
            })
    );
    const size = createElementSize(document.getElementById("root")!);
    createEffect(() => {
        console.log({ width: size.width, height: size.height });
        invoke("content_size_changed", {
            width: size.width,
            height: size.height,
        });
    });

    return (
        <div class="px-4 pt-2 border-slate-700 border-2">
            <For each={definitions()}>
                {(definition, _) => <Definition definition={definition} />}
            </For>
        </div>
    );
}

function Definition(props: { definition: Definition }) {
    return (
        <div class="mb-2">
            <p class="font-medium text-lg">{props.definition.simplified}</p>
            <div class="flex flex-row space-x-1">
                <For each={props.definition.pinyin}>
                    {(pinyin, _) => (
                        <p class={classForTone(pinyin.tone)}>
                            {pinyin.syllable}
                        </p>
                    )}
                </For>
            </div>
            <div class="flex flex-col divide-y">
                <For each={props.definition.translations}>
                    {(translation, _) => (
                        <p class="py-1 font-light text-nowrap">{translation}</p>
                    )}
                </For>
            </div>
        </div>
    );
}

function classForTone(tone: number): string {
    switch (tone) {
        case 1:
            return "text-[#268bd2] dark:text-[#6c71c4]";
        case 2:
            return "text-[#b58900] dark:text-[#cb4b16]";
        case 3:
            return "text-[#859900] dark:text-[#2aa198]";
        case 4:
            return "text-[#d33682] dark:text-[#dc322f]";
        case 5:
            return "text-[#586e75] dark:text-[#93a1a1]";
        default:
            return "text-black dark:text-white";
    }
}

export default App;
