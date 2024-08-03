/* @refresh reload */
import { render } from "solid-js/web";

import "./index.css";
import Tooltip from "./Tooltip";

const root = document.getElementById("root");

render(() => <Tooltip />, root!);
