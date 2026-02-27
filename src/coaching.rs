use crate::models::{TrainingPlan, TrainingTarget, WorkoutType};
use chrono::{Duration, Utc};

pub struct CoachContext {
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub available_equipment: Vec<String>,
}

pub struct BriefInput<'a> {
    pub detailed_activities: &'a [crate::models::GarminActivity],
    pub plans: &'a [crate::models::GarminPlan],
    pub profile: &'a Option<crate::models::GarminProfile>,
    pub metrics: &'a Option<crate::models::GarminMaxMetrics>,
    pub scheduled_workouts: &'a [crate::models::ScheduledWorkout],
    pub recovery_metrics: &'a Option<crate::models::GarminRecoveryMetrics>,
    pub context: &'a CoachContext,
    pub progression_history: &'a [String],
}

pub struct Coach;

impl Coach {
    pub fn new() -> Self {
        Coach
    }

    #[allow(dead_code)]
    pub fn generate_smart_plan(
        &self,
        detailed_activities: &[crate::models::GarminActivity],
    ) -> TrainingPlan {
        let now = Utc::now();
        let week_start = now - Duration::days(7);
        let week_start_str = week_start.format("%Y-%m-%dT%H:%M:%S").to_string();

        // Analyze recent history (last 7 days)
        let recent_activities: Vec<&crate::models::GarminActivity> = detailed_activities
            .iter()
            .filter(|a| a.start_time > week_start_str)
            .collect();

        let bike_count = recent_activities
            .iter()
            .filter(|a| {
                let s = a.get_activity_type().unwrap_or("unknown").to_lowercase();
                s.contains("cycling") || s.contains("biking")
            })
            .count();

        let run_count = recent_activities
            .iter()
            .filter(|a| {
                a.get_activity_type()
                    .unwrap_or("unknown")
                    .to_lowercase()
                    .contains("running")
            })
            .count();

        let strength_count = recent_activities
            .iter()
            .filter(|a| {
                let s = a.get_activity_type().unwrap_or("unknown").to_lowercase();
                s.contains("strength") || s.contains("fitness")
            })
            .count();

        // Analyze Strength Volume from Detailed Data
        let mut strength_volume_kg = 0.0;
        let week_start_str = week_start.format("%Y-%m-%dT%H:%M:%S").to_string();
        for da in detailed_activities {
            if da.start_time > week_start_str {
                if let Some(crate::models::GarminSetsData::Details(data)) = &da.sets {
                    let vol: f64 = data
                        .exercise_sets
                        .iter()
                        .filter(|s| s.set_type == "ACTIVE")
                        .map(|s| {
                            s.weight.unwrap_or(0.0) / 1000.0
                                * (s.repetition_count.unwrap_or(0) as f64)
                        })
                        .sum();
                    strength_volume_kg += vol;
                }
            }
        }

        println!(
            "Recent Activity (Last 7d): Bike: {}, Run: {}, Strength: {} (Vol: {:.0}kg)",
            bike_count, run_count, strength_count, strength_volume_kg
        );

        let mut workouts = Vec::new();
        let end_of_week = now + Duration::days(7);

        // --- Biking Logic ---
        if bike_count < 1 {
            // Boot up / Base Phase
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 45.0,
                target_distance_km: None,
                description: "Base Builder: Easy spin to get back into rhythm. Zone 1-2."
                    .to_string(),
            });
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 60.0,
                target_distance_km: None,
                description: "Endurance: Steady ride, focus on cadence.".to_string(),
            });
        } else {
            // Progression
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 60.0,
                target_distance_km: None,
                description: "Hill Repeats: 4x 5min climbing at threshold.".to_string(),
            });
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 90.0,
                target_distance_km: None,
                description: "Mountain Endurance: Long steady climb simulation.".to_string(),
            });
        }

        // --- Strength Logic ---
        // Volume check for coaching advice
        let strength_focus = if strength_volume_kg > 5000.0 {
            "Deload / Technique Focus: Keep weights light, focus on mobility."
        } else {
            "Progression: Aim to increase weight or reps."
        };

        if strength_count < 2 {
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Strength,
                target_duration_minutes: 45.0,
                target_distance_km: None,
                description: format!(
                    "Full Body A: Squats, Pushups, Rows, Core. {}",
                    strength_focus
                ),
            });
        }
        workouts.push(TrainingTarget {
            workout_type: WorkoutType::Strength,
            target_duration_minutes: 45.0,
            target_distance_km: None,
            description: format!(
                "Full Body B: Deadlifts, Overhead Press, Lunges. {}",
                strength_focus
            ),
        });

        // --- Running Note ---
        // We don't schedule running (Garmin Coach does), but we acknowledge it.
        if run_count > 2 {
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Unknown,
                target_duration_minutes: 0.0,
                target_distance_km: None,
                description:
                    "Note: High running volume detected. Ensure bike rides are low impact."
                        .to_string(),
            });
        }

        TrainingPlan {
            start_date: now,
            end_date: end_of_week,
            workouts,
        }
    }

    pub fn generate_brief(&self, input: BriefInput<'_>) -> String {
        let BriefInput {
            detailed_activities,
            plans,
            profile,
            metrics,
            scheduled_workouts,
            recovery_metrics,
            context,
            progression_history,
        } = input;
        let now = Utc::now();
        let mut brief = String::new();

        // 1. Header & Current Context
        brief.push_str("# Certified Coaching Brief\n\n");
        brief.push_str("**Role**: You are an elite Multi-Sport Coach (Triathlon/Strength/Endurance). Your job is to analyze the athlete's data and produce a highly specific, periodized training plan.\n\n");

        let today_date_str = now.format("%Y-%m-%d").to_string();
        brief.push_str(&format!("**Current Date**: {}\n\n", today_date_str));

        // Let's summarize what was already done today from the history
        brief.push_str("**Activities Completed Today**:\n");
        let todays_activities: Vec<&crate::models::GarminActivity> = detailed_activities
            .iter()
            .filter(|a| a.start_time.starts_with(&today_date_str))
            .collect();

        if todays_activities.is_empty() {
            brief.push_str("- None.\n\n");
        } else {
            for a in todays_activities {
                let dur = a.duration.unwrap_or(0.0) / 60.0;
                let dist = a.distance.unwrap_or(0.0) / 1000.0;
                brief.push_str(&format!(
                    "- **{}**: {:.1} min, {:.1} km\n",
                    a.name.as_deref().unwrap_or("Unknown"),
                    dur,
                    dist
                ));
            }
            brief.push('\n');
        }

        if let Some(rec) = recovery_metrics {
            brief.push_str("**Today's Recovery & Readiness**:\n");
            if let Some(bb) = rec.current_body_battery {
                brief.push_str(&format!("- **Body Battery**: {} / 100\n", bb));
            }
            if let Some(tr) = rec.training_readiness {
                brief.push_str(&format!("- **Training Readiness**: {} / 100\n", tr));
            }
            if let Some(hrv) = &rec.hrv_status {
                brief.push_str(&format!("- **HRV Status**: {}\n", hrv));
            }
            if let Some(ss) = rec.sleep_score {
                brief.push_str(&format!("- **Sleep Score**: {} / 100\n", ss));
            }

            if !rec.recent_sleep_scores.is_empty() {
                brief.push_str("- **7-Day Sleep Trend**: ");
                let trend_strs: Vec<String> = rec
                    .recent_sleep_scores
                    .iter()
                    .map(|s| {
                        format!(
                            "{} ({})",
                            s.score,
                            s.date.chars().skip(5).collect::<String>()
                        )
                    })
                    .collect();
                brief.push_str(&trend_strs.join(", "));
                brief.push('\n');
            }

            brief.push('\n');
        }

        // 2. Athlete Profile
        brief.push_str("## Athlete Profile\n");
        if let Some(p) = profile {
            if let Some(w) = p.weight {
                brief.push_str(&format!("- **Weight**: {:.1} kg\n", w / 1000.0));
            } // Weight is in grams usually? Check Garmin output. Output says 72500.0, so yes grams.
            if let Some(h) = p.height {
                brief.push_str(&format!("- **Height**: {:.1} cm\n", h));
            }
            if let Some(dob) = &p.birth_date {
                brief.push_str(&format!("- **DOB**: {}\n", dob));
            }
            if let Some(v) = p.vo2_max_running {
                brief.push_str(&format!("- **VO2Max (Run)**: {:.1}\n", v));
            }
        }
        if let Some(m) = metrics {
            if let Some(v) = m.vo2_max_precise {
                brief.push_str(&format!("- **VO2Max (Precise)**: {:.1}\n", v));
            }
            if let Some(fa) = m.fitness_age {
                brief.push_str(&format!("- **Fitness Age**: {}\n", fa));
            }
        }
        brief.push('\n');

        // 3. Goals & Constraints
        brief.push_str("## Goals & Context\n");
        brief.push_str("**Primary Goals**:\n");
        for g in &context.goals {
            brief.push_str(&format!("- [ ] {}\n", g));
        }

        brief.push_str("\n**Available Equipment**:\n");
        for e in &context.available_equipment {
            brief.push_str(&format!("- {}\n", e));
        }

        brief.push_str("\n**Active Training Cycles (Garmin Coach)**:\n");
        if plans.is_empty() {
            brief.push_str("- None active.\n");
        } else {
            for p in plans {
                brief.push_str(&format!(
                    "- **{}** (Type: {}, Ends: {})\n",
                    p.name, p.plan_type, p.end_date
                ));
            }
        }

        let mut upcoming_races = Vec::new();
        let mut upcoming_workouts = Vec::new();

        for sw in scheduled_workouts {
            if let Some(ref it) = sw.item_type {
                if it == "race" || it == "event" || it == "primaryEvent" {
                    upcoming_races.push(sw);
                } else {
                    upcoming_workouts.push(sw);
                }
            } else {
                upcoming_workouts.push(sw);
            }
        }

        if !upcoming_races.is_empty() {
            brief.push_str("\n**CRITICAL: Upcoming Races & Events**:\n");
            for race in upcoming_races {
                let mut details = format!(
                    "- **{}** (Date: {}, Sport: {}",
                    race.title.as_deref().unwrap_or("Untitled Event"),
                    race.date,
                    race.sport.as_deref().unwrap_or("Unknown")
                );
                if let Some(dist) = race.distance {
                    details.push_str(&format!(", Distance: {:.1}km", dist));
                }
                details.push_str(")\n");
                brief.push_str(&details);
            }
            brief.push_str("*Note for Coach*: You MUST take note of these upcoming Races or Events. Adjust the training volume to taper appropriately leading up to the event date, ensuring the athlete peaks for the race.\n");
        }

        brief.push_str("\n**Scheduled Garmin Workouts**:\n");
        if upcoming_workouts.is_empty() {
            brief.push_str("- None scheduled.\n");
        } else {
            for sw in upcoming_workouts {
                let mut details = format!(
                    "- **{}** (Date: {}, Sport: {}",
                    sw.title.as_deref().unwrap_or("Untitled"),
                    sw.date,
                    sw.sport.as_deref().unwrap_or("Unknown")
                );
                if let Some(d) = sw.duration {
                    details.push_str(&format!(", Duration: {:.0}min", d));
                }
                if let Some(dist) = sw.distance {
                    details.push_str(&format!(", Distance: {:.1}km", dist));
                }
                if let Some(desc) = &sw.description {
                    details.push_str(&format!(", Focus: '{}'", desc));
                }
                details.push_str(")\n");
                brief.push_str(&details);
            }
            brief.push_str("\n*Note for Coach*: Please consider the scheduled Garmin workouts above. Advise if today's scheduled workout should be performed, and adjust the strength volume if necessary.\n");
        }
        brief.push_str("\n**Constraints**:\n");
        for c in &context.constraints {
            brief.push_str(&format!("- {}\n", c));
        }
        brief.push('\n');

        // 4. Status Update (30 Days)
        let thirty_days_ago = now - Duration::days(30);
        let thirty_days_ago_str = thirty_days_ago.format("%Y-%m-%dT%H:%M:%S").to_string();

        let recent_30d: Vec<&crate::models::GarminActivity> = detailed_activities
            .iter()
            .filter(|a| a.start_time >= thirty_days_ago_str)
            .collect();

        let _total_count = recent_30d.len();
        let total_dist_km: f64 = recent_30d
            .iter()
            .map(|a| a.distance.unwrap_or(0.0) / 1000.0)
            .sum();
        let total_dur_min: f64 = recent_30d
            .iter()
            .map(|a| a.duration.unwrap_or(0.0) / 60.0)
            .sum();

        brief.push_str("## Training Status (Last 30 Days)\n");
        brief.push_str(&format!(
            "- **Volume**: {:.1} km / {:.1} hours\n",
            total_dist_km,
            total_dur_min / 60.0
        ));

        let run_count = recent_30d
            .iter()
            .filter(|a| {
                a.get_activity_type()
                    .unwrap_or("unknown")
                    .to_lowercase()
                    .contains("run")
            })
            .count();
        let bike_count = recent_30d
            .iter()
            .filter(|a| {
                let s = a.get_activity_type().unwrap_or("unknown").to_lowercase();
                s.contains("bike") || s.contains("cycl")
            })
            .count();
        let strength_count = recent_30d
            .iter()
            .filter(|a| {
                a.get_activity_type()
                    .unwrap_or("unknown")
                    .to_lowercase()
                    .contains("strength")
                    || a.get_activity_type()
                        .unwrap_or("unknown")
                        .to_lowercase()
                        .contains("fitness")
            })
            .count();
        brief.push_str(&format!(
            "- **Frequency**: {} Runs, {} Rides, {} Strength sessions\n",
            run_count, bike_count, strength_count
        ));

        // 5. Detailed Recent Log (Last 14 Days for deeper context)
        let cutoff = now - Duration::days(14);
        let _cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%S").to_string();

        // 4. Activity Log (Last 14d)
        brief.push_str("\n## Activity Log (Last 14 Days)\n");

        let two_weeks_ago = now - Duration::days(14);
        let _two_weeks_ago_str = two_weeks_ago.format("%Y-%m-%dT%H:%M:%S").to_string();

        let mut weekly_muscle_volume: std::collections::HashMap<&str, i32> =
            std::collections::HashMap::new();

        // Sort detailed activities by date desc
        let _sorted_activities = detailed_activities.to_vec();

        // Take up to 20 most recent activities from the detailed array
        let mut count = 0;
        for act in detailed_activities {
            let act_time = chrono::DateTime::parse_from_rfc3339(&act.start_time)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&act.start_time, "%Y-%m-%d %H:%M:%S")
                        .map(|ndt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
                        .unwrap_or_default()
                });
            if act_time > two_weeks_ago {
                let mut focus_str = String::new();
                if let Some(crate::models::GarminSetsData::Details(data)) = &act.sets {
                    // Extract unique exercise categories
                    let mut unique_exercises = std::collections::HashSet::new();

                    let is_last_7_days = act_time > (now - Duration::days(7));

                    for set in &data.exercise_sets {
                        if let Some(ex) = set.exercises.first() {
                            unique_exercises.insert(ex.category.clone());

                            // Accumulate muscle group volume for the last 7 days
                            // We only count ACTIVE working sets
                            if is_last_7_days && set.set_type == "ACTIVE" {
                                if ex.category == "WARM_UP" {
                                    continue;
                                }
                                let mg = match ex.category.as_str() {
                                    "BENCH_PRESS" | "PUSH_UP" => "Chest",
                                    "ROW" | "PULL_UP" | "PULL_DOWN" => "Back",
                                    "SQUAT" | "DEADLIFT" | "LUNGE" | "CALF_RAISE" => "Legs",
                                    "SHOULDER_PRESS" | "FRONT_RAISE" | "LATERAL_RAISE" => {
                                        "Shoulders"
                                    }
                                    "TRICEPS_EXTENSION" | "BICEP_CURL" => "Arms",
                                    "CORE" | "PLANK" | "SIT_UP" => "Core",
                                    _ => "Other",
                                };
                                *weekly_muscle_volume.entry(mg).or_insert(0) += 1;
                            }
                        }
                    }
                    if !unique_exercises.is_empty() {
                        let sorted: Vec<_> = unique_exercises.into_iter().collect();
                        focus_str = format!(". Focus: {}", sorted.join(", "));
                    }
                }

                let vol_str = if focus_str.is_empty() {
                    "".to_string()
                } else {
                    let mut vol = 0.0;
                    if let Some(crate::models::GarminSetsData::Details(data)) = &act.sets {
                        vol = data
                            .exercise_sets
                            .iter()
                            .filter(|s| s.set_type == "ACTIVE")
                            .map(|s| {
                                s.weight.unwrap_or(0.0) / 1000.0
                                    * (s.repetition_count.unwrap_or(0) as f64)
                            })
                            .sum();
                    }
                    format!(", Vol: {:.0} kg", vol)
                };

                brief.push_str(&format!(
                    "- **{} {}**: {:.1} min, {:.1} km{}{} , Avg HR: {:.0}\n",
                    act.start_time.split('T').next().unwrap_or(""),
                    act.name.as_deref().unwrap_or("Unknown"),
                    act.duration.unwrap_or(0.0) / 60.0,
                    act.distance.unwrap_or(0.0) / 1000.0,
                    vol_str,
                    focus_str,
                    act.average_hr.unwrap_or(0.0)
                ));
                count += 1;
                if count >= 20 {
                    break;
                }
            }
        }
        brief.push('\n');

        if !progression_history.is_empty() {
            brief.push_str(
                "## Current Progression Track (All-Time Bests / Recent Working Weights)\n",
            );
            brief.push_str("*Max weight recorded used as baseline for progressive overload.*\n");
            for entry in progression_history {
                brief.push_str(&format!("{}\n", entry));
            }
        }
        // 5. Muscle Fatigue Heatmap
        if !weekly_muscle_volume.is_empty() {
            brief.push_str("## Muscle Fatigue Heatmap (Last 7 Days)\n");
            brief.push_str("*Number of Active Working Sets performed per muscle group. Aim for 10-20 sets per week for optimal hypertrophy.* \n");
            let mut sorted_volumes: Vec<_> = weekly_muscle_volume.iter().collect();
            sorted_volumes.sort_by(|a, b| b.1.cmp(a.1)); // Sort descending by volume
            for (mg, vol) in sorted_volumes {
                brief.push_str(&format!("- **{}**: {} sets\n", mg, vol));
            }
            brief.push('\n');
        }

        // 6. Required Output
        brief.push_str("## Required Output\n");
        brief.push_str("Based on the Athlete Profile, Goals, and Activity Log, please generate the training plan for the **Next 7 Days**.\n");
        brief.push_str("You **MUST** output the Strength Workouts in the following JSON format (inside a json code block). \n");
        brief.push_str("**CRITICAL RULES**:\n");
        brief.push_str(
            "1. Start every workout with a Dynamic Warmup and end with Static Stretching.\n",
        );
        brief.push_str("2. **EXERCISE VOCABULARY**: Our system automatically maps your exercises to the Garmin database. You may use any standard exercise name (e.g. 'Barbell Bench Press', 'Goblet Squat', 'Pull Up', 'Dumbbell Hammer Curl', etc.). The system will find the closest match. Try to be as specific as possible.\n");
        brief.push_str("3. **REST PERIODS**: For the `rest` field, output an integer in seconds (e.g., `rest: 90`), or the exact string `\"LAP\"` if the rest should remain untimed until the user manually presses the lap button.\n");
        brief.push_str("4. **SCHEDULE**: Include a `scheduledDate` field at the top level of each workout, formatted as \"YYYY-MM-DD\". Assign the date you logically want the athlete to perform this (distributed across the next 7 days starting from tomorrow).\n");

        brief.push_str("\n```json\n");
        brief.push_str("[\n");
        brief.push_str("  {\n");
        brief.push_str("    \"workoutName\": \"Strength A - Push Focus\",\n");
        brief.push_str("    \"description\": \"Focus on chest and triceps hypertrophy.\",\n");
        brief.push_str("    \"scheduledDate\": \"2026-02-21\",\n");
        brief.push_str("    \"steps\": [\n");
        brief.push_str("      { \"phase\": \"warmup\", \"exercise\": \"ROW\", \"duration\": \"5min\", \"note\": \"Light rowing or cardio.\" },\n");
        brief.push_str("      { \"phase\": \"interval\", \"exercise\": \"BENCH_PRESS\", \"weight\": 12.5, \"reps\": 10, \"sets\": 4, \"rest\": 120, \"note\": \"Progressive overload from last week.\" },\n");
        brief.push_str("      { \"phase\": \"interval\", \"exercise\": \"SHOULDER_PRESS\", \"weight\": 10.0, \"reps\": \"AMRAP\", \"sets\": 3, \"rest\": \"LAP\", \"note\": \"Push to near failure.\" },\n");
        brief.push_str("      { \"phase\": \"cooldown\", \"exercise\": \"YOGA\", \"duration\": \"10min\", \"note\": \"Static stretching for chest and tris.\" }\n");
        brief.push_str("    ]\n");
        brief.push_str("  }\n");
        brief.push_str("]\n");
        brief.push_str("```\n");
        brief.push_str("Use `phase`: 'warmup', 'interval', or 'cooldown'. For 'weight', ensure you propose a specific load (in kg) available in the equipment list. For 'reps', use integers or 'AMRAP'.\n");

        brief
    }
}
