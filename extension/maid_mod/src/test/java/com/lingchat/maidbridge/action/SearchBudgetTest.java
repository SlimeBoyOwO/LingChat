package com.lingchat.maidbridge.action;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class SearchBudgetTest {
    @Test
    void handsOutExactlyWhatItHas() {
        SearchBudget budget = new SearchBudget(2);
        assertTrue(budget.take());
        assertTrue(budget.take());
        assertTrue(budget.spent());
        assertFalse(budget.take());
    }

    @Test
    void emptyBudgetNeverGives() {
        SearchBudget budget = new SearchBudget(0);
        assertTrue(budget.spent());
        assertFalse(budget.take());
    }

    @Test
    void negativeBecomesEmpty() {
        SearchBudget budget = new SearchBudget(-3);
        assertTrue(budget.spent());
        assertFalse(budget.take());
    }
}
