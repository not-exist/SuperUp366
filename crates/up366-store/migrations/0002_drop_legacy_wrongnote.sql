-- The legacy wrong-note API and its cleanup flow are gone; these tables only
-- held its cache and run history.
DROP TABLE IF EXISTS wrongnote_clean_items;
DROP TABLE IF EXISTS wrongnote_clean_runs;
DROP TABLE IF EXISTS wrongnote_items;
