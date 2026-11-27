import { answerThroughUi } from "./node_modules/gentle-pi/extensions/gentle-agents.ts";
export default function register(pi) {
  pi.registerCommand("ivy-probe", {
    description: "Exercise the installed Gentle helper dialog relay",
    handler: async (_, ctx) => {
      const reply = await answerThroughUi(ctx.ui, {method:"select", title:"Choose a route"}, {options:["Left", "Right"]});
      ctx.ui.notify("ivy-probe " + JSON.stringify(reply));
    },
  });
}
