-- Read-aloud bookkeeping.
--
-- `paper_id` lets the read-aloud view resolve the paper without parsing the
-- cached raw JSON on every open.
ALTER TABLE jobs ADD COLUMN paper_id TEXT;

-- The URL the server will read out of `<answer_url>`. `object_name` alone is
-- not enough to rebuild it: the bucket/host come from the STS response.
ALTER TABLE audio_assets ADD COLUMN object_url TEXT;
