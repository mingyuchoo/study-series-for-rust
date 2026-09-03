// AMAP agent governance (design §10). Cedar answers "who may do what";
// the Invariant DSL answers "what must always be true".

// Default: agents may act; the forbids below carve out the guardrails.
permit(principal, action, resource);

// An agent must never approve or review its own changes.
forbid(principal, action in [Action::"approve", Action::"review"], resource)
when { context has author && context.author == principal };

// Critical functions require an independent verifier before merge / certification.
forbid(principal, action in [Action::"merge", Action::"certify"], resource)
when { context.is_critical && !context.independent_verifier };

// High uncertainty (> 20%, expressed in basis points) requires human approval.
forbid(principal, action in [Action::"merge", Action::"certify"], resource)
when { context.uncertainty_bp > 2000 && !context.human_approved };

// The final PASS / FAIL decision is never made by an LLM-backed agent.
forbid(principal is Agent, action == Action::"decide_pass_fail", resource);

// The RCA agent proposes hypotheses; it does not patch code.
forbid(principal == Agent::"rca", action == Action::"patch", resource);

// Builders never write verification tests that will judge their own output.
forbid(principal == Agent::"builder", action == Action::"write_verification_test", resource);
