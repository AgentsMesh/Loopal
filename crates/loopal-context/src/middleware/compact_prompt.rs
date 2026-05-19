pub const NO_TOOLS_PREAMBLE: &str = "\
You MUST NOT call any tools.\n\
You MUST NOT request follow-up actions.\n\
Your only output is the structured summary defined below.\n";

pub const SYSTEM_PROMPT: &str = "You produce structured working state summaries \
for coding agents. Be factual, terse, and preserve every identifier verbatim.";

const SECTIONS_HEADER: &str = "\
Produce a WORKING STATE document. The agent will continue using ONLY this \
document plus the most recent messages. The original conversation will not \
be available afterwards.\n\n\
Write your draft inside <analysis>...</analysis>. Then write the final \
summary inside <summary>...</summary>. Only the <summary> contents are \
persisted; <analysis> is discarded.\n\n\
The <summary> block MUST contain these 9 sections, in order:\n\
1. Primary Request and Intent — quote the user's exact original ask.\n\
2. Key Technical Concepts — frameworks, protocols, data structures involved.\n\
3. Files and Code Sections — every path touched and what changed there.\n\
4. Errors and Fixes — every failure encountered and how it was resolved.\n\
5. Problem Solving — non-trivial reasoning the agent did.\n\
6. All User Messages — verbatim list of every user turn so far.\n\
7. Pending Tasks — what the user asked for that is still incomplete.\n\
8. Current Work — exactly what was being worked on at the boundary.\n\
9. Optional Next Step — single most natural continuation, only if obvious.\n\n\
Rules:\n\
- Quote identifiers (functions, paths, error messages) verbatim.\n\
- Do not embed file contents — only paths + a one-line description.\n\
- Use bullet lists, not prose paragraphs.\n";

pub fn build_prompt(conversation_text: &str, custom_instructions: Option<&str>) -> String {
    let mut prompt = String::with_capacity(conversation_text.len() + 2048);
    prompt.push_str(NO_TOOLS_PREAMBLE);
    prompt.push('\n');
    prompt.push_str(SECTIONS_HEADER);
    if let Some(extra) = custom_instructions {
        let extra = extra.trim();
        if !extra.is_empty() {
            prompt.push_str("\n<custom-instructions>\n");
            prompt.push_str(extra);
            prompt.push_str("\n</custom-instructions>\n");
        }
    }
    prompt.push_str("\nConversation:\n---\n");
    prompt.push_str(conversation_text);
    prompt.push_str("\n---");
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_all_nine_sections() {
        let p = build_prompt("conv", None);
        for n in 1..=9 {
            assert!(p.contains(&format!("{n}.")), "missing section {n}: {p}");
        }
    }

    #[test]
    fn prompt_includes_no_tools_preamble() {
        let p = build_prompt("conv", None);
        assert!(p.contains("MUST NOT call any tools"));
    }

    #[test]
    fn prompt_includes_tags() {
        let p = build_prompt("conv", None);
        assert!(p.contains("<analysis>"));
        assert!(p.contains("<summary>"));
    }

    #[test]
    fn prompt_embeds_conversation() {
        let p = build_prompt("HELLO_CONV", None);
        assert!(p.contains("HELLO_CONV"));
    }

    #[test]
    fn custom_instructions_injected() {
        let p = build_prompt("c", Some("preserve test repro steps"));
        assert!(p.contains("<custom-instructions>"));
        assert!(p.contains("preserve test repro steps"));
    }

    #[test]
    fn empty_custom_instructions_omitted() {
        let p = build_prompt("c", Some("   "));
        assert!(!p.contains("<custom-instructions>"));
    }
}
