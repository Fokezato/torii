-- "Parar de semear" num episódio vale de vez: não volta a semear ao reabrir.
ALTER TABLE episodes ADD COLUMN seeding_stopped INTEGER NOT NULL DEFAULT 0;
