package mcrs.data;

import static mcrs.data.CodecGoldens.hex;

import com.google.gson.JsonParser;
import com.mojang.serialization.DataResult;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import it.unimi.dsi.fastutil.ints.Int2ObjectMap;
import it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.Set;
import java.util.function.Supplier;
import mcrs.data.CodecGoldens.Session;
import net.minecraft.SharedConstants;
import net.minecraft.advancements.Advancement;
import net.minecraft.advancements.AdvancementHolder;
import net.minecraft.advancements.AdvancementProgress;
import net.minecraft.advancements.AdvancementRewards;
import net.minecraft.advancements.AdvancementRequirements;
import net.minecraft.advancements.AdvancementType;
import net.minecraft.advancements.CriterionProgress;
import net.minecraft.advancements.DisplayInfo;
import net.minecraft.core.BlockPos;
import net.minecraft.core.ClientAsset;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderSet;
import net.minecraft.core.component.DataComponentExactPredicate;
import net.minecraft.core.component.DataComponentPatch;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.TypedDataComponent;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.particles.ParticleType;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.HashedPatchMap;
import net.minecraft.network.HashedStack;
import net.minecraft.network.chat.Component;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.protocol.game.ClientboundContainerClosePacket;
import net.minecraft.network.protocol.game.ClientboundContainerSetContentPacket;
import net.minecraft.network.protocol.game.ClientboundContainerSetDataPacket;
import net.minecraft.network.protocol.game.ClientboundContainerSetSlotPacket;
import net.minecraft.network.protocol.game.ClientboundLevelParticlesPacket;
import net.minecraft.network.protocol.game.ClientboundLoginPacket;
import net.minecraft.network.protocol.game.ClientboundMerchantOffersPacket;
import net.minecraft.network.protocol.game.ClientboundOpenScreenPacket;
import net.minecraft.network.protocol.game.ClientboundRecipeBookAddPacket;
import net.minecraft.network.protocol.game.ClientboundRecipeBookRemovePacket;
import net.minecraft.network.protocol.game.ClientboundRecipeBookSettingsPacket;
import net.minecraft.network.protocol.game.ClientboundRespawnPacket;
import net.minecraft.network.protocol.game.ClientboundSetCursorItemPacket;
import net.minecraft.network.protocol.game.ClientboundSetHeldSlotPacket;
import net.minecraft.network.protocol.game.ClientboundSetPlayerInventoryPacket;
import net.minecraft.network.protocol.game.ClientboundTakeItemEntityPacket;
import net.minecraft.network.protocol.game.ClientboundUpdateAdvancementsPacket;
import net.minecraft.network.protocol.game.ClientboundUpdateRecipesPacket;
import net.minecraft.network.protocol.game.CommonPlayerSpawnInfo;
import net.minecraft.network.protocol.game.ServerboundContainerButtonClickPacket;
import net.minecraft.network.protocol.game.ServerboundContainerClickPacket;
import net.minecraft.network.protocol.game.ServerboundContainerClosePacket;
import net.minecraft.network.protocol.game.ServerboundContainerSlotStateChangedPacket;
import net.minecraft.network.protocol.game.ServerboundEditBookPacket;
import net.minecraft.network.protocol.game.ServerboundPickItemFromBlockPacket;
import net.minecraft.network.protocol.game.ServerboundPickItemFromEntityPacket;
import net.minecraft.network.protocol.game.ServerboundPlaceRecipePacket;
import net.minecraft.network.protocol.game.ServerboundRecipeBookChangeSettingsPacket;
import net.minecraft.network.protocol.game.ServerboundRecipeBookSeenRecipePacket;
import net.minecraft.network.protocol.game.ServerboundRenameItemPacket;
import net.minecraft.network.protocol.game.ServerboundSeenAdvancementsPacket;
import net.minecraft.network.protocol.game.ServerboundSelectTradePacket;
import net.minecraft.network.protocol.game.ServerboundSetCreativeModeSlotPacket;
import net.minecraft.network.protocol.handshake.ClientIntent;
import net.minecraft.network.protocol.handshake.ClientIntentionPacket;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.stats.RecipeBookSettings;
import net.minecraft.tags.ItemTags;
import net.minecraft.util.Unit;
import net.minecraft.world.inventory.ContainerInput;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.inventory.RecipeBookType;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ItemStackTemplate;
import net.minecraft.world.item.crafting.Ingredient;
import net.minecraft.world.item.crafting.RecipeBookCategories;
import net.minecraft.world.item.crafting.RecipeBookCategory;
import net.minecraft.world.item.crafting.RecipePropertySet;
import net.minecraft.world.item.crafting.SelectableRecipe;
import net.minecraft.world.item.crafting.StonecutterRecipe;
import net.minecraft.world.item.crafting.display.FurnaceRecipeDisplay;
import net.minecraft.world.item.crafting.display.RecipeDisplay;
import net.minecraft.world.item.crafting.display.RecipeDisplayEntry;
import net.minecraft.world.item.crafting.display.RecipeDisplayId;
import net.minecraft.world.item.crafting.display.ShapedCraftingRecipeDisplay;
import net.minecraft.world.item.crafting.display.ShapelessCraftingRecipeDisplay;
import net.minecraft.world.item.crafting.display.SlotDisplay;
import net.minecraft.world.item.crafting.display.SmithingRecipeDisplay;
import net.minecraft.world.item.crafting.display.StonecutterRecipeDisplay;
import net.minecraft.world.item.equipment.trim.TrimPattern;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.item.trading.MerchantOffer;
import net.minecraft.world.item.trading.MerchantOffers;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.AbstractFurnaceBlock;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.dimension.DimensionType;

public final class PacketGoldens {
    private static final Map<String, String> PARSE_ERROR_INPUTS = Map.of(
        "geyser_zero", "{\"type\":\"minecraft:geyser\",\"water_blocks\":0}",
        "trail_zero", "{\"type\":\"minecraft:trail\",\"target\":[0,0,0],\"color\":1,\"duration\":0}"
    );

    private PacketGoldens() {}

    private static void rewriteLabelled(
        final Session session, final Path current, final Path output, final Map<String, Supplier<String>> labels
    ) throws Exception {
        List<String> out = new ArrayList<>();
        Set<String> used = new LinkedHashSet<>();
        for (String line : Files.readAllLines(current, StandardCharsets.UTF_8)) {
            if (line.startsWith("id ")) {
                out.add(CodecGoldens.idLine(session, line, false));
                continue;
            }
            String label = line.substring(0, line.indexOf(' '));
            Supplier<String> source = labels.get(label);
            if (source == null) {
                throw new IllegalStateException("no inputs for label " + label);
            }
            used.add(label);
            out.add(label + " " + source.get());
        }
        if (!used.equals(labels.keySet())) {
            throw new IllegalStateException("labels with inputs but not in the file: " + labels.keySet().stream().filter(label -> !used.contains(label)).toList());
        }
        CodecGoldens.writeLines(output, out);
    }

    static void recipePackets(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            Map<String, Supplier<String>> labels = new LinkedHashMap<>();
            labels.put("recipe_book_add", () -> "wire " + hex(session, ClientboundRecipeBookAddPacket.STREAM_CODEC, recipeBookAdd(session)));
            labels.put("update_recipes", () -> "wire " + hex(session, ClientboundUpdateRecipesPacket.STREAM_CODEC, updateRecipes(session)));
            labels.put(
                "recipe_book_remove",
                () -> "wire " + hex(
                    session,
                    ClientboundRecipeBookRemovePacket.STREAM_CODEC,
                    new ClientboundRecipeBookRemovePacket(List.of(new RecipeDisplayId(1), new RecipeDisplayId(300)))
                )
            );
            labels.put(
                "recipe_book_settings",
                () -> "wire " + hex(session, ClientboundRecipeBookSettingsPacket.STREAM_CODEC, new ClientboundRecipeBookSettingsPacket(recipeBookSettings()))
            );
            labels.put(
                "place_recipe",
                () -> "wire " + hex(session, ServerboundPlaceRecipePacket.STREAM_CODEC, new ServerboundPlaceRecipePacket(7, new RecipeDisplayId(300), true))
            );
            labels.put(
                "recipe_book_change_settings",
                () -> "wire " + hex(
                    session,
                    ServerboundRecipeBookChangeSettingsPacket.STREAM_CODEC,
                    new ServerboundRecipeBookChangeSettingsPacket(RecipeBookType.BLAST_FURNACE, true, false)
                )
            );
            labels.put(
                "recipe_book_seen_recipe",
                () -> "wire " + hex(session, ServerboundRecipeBookSeenRecipePacket.STREAM_CODEC, new ServerboundRecipeBookSeenRecipePacket(new RecipeDisplayId(9)))
            );
            rewriteLabelled(session, current, output, labels);
        });
    }

    static void inventoryPackets(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            Map<String, Supplier<String>> labels = new LinkedHashMap<>();
            labels.put(
                "container_set_slot",
                () -> hex(session, ClientboundContainerSetSlotPacket.STREAM_CODEC, new ClientboundContainerSetSlotPacket(3, 7, 5, sword(session)))
            );
            labels.put(
                "container_set_slot_empty",
                () -> hex(session, ClientboundContainerSetSlotPacket.STREAM_CODEC, new ClientboundContainerSetSlotPacket(0, 1, -1, ItemStack.EMPTY))
            );
            labels.put("container_set_content", () -> {
                List<ItemStack> player = emptyStacks(46);
                player.set(9, stack(session, "apple", 3));
                player.set(36, sword(session));
                player.set(45, stack(session, "stone", 16));
                return hex(
                    session, ClientboundContainerSetContentPacket.STREAM_CODEC, new ClientboundContainerSetContentPacket(0, 5, player, stack(session, "stone", 64))
                );
            });
            labels.put("container_set_content_chest", () -> {
                List<ItemStack> chest = emptyStacks(63);
                chest.set(0, stack(session, "diamond", 5));
                chest.set(26, stack(session, "emerald", 1));
                chest.set(27, stack(session, "apple", 2));
                chest.set(62, stack(session, "stone", 1));
                return hex(
                    session, ClientboundContainerSetContentPacket.STREAM_CODEC, new ClientboundContainerSetContentPacket(1, 2, chest, ItemStack.EMPTY)
                );
            });
            labels.put(
                "set_cursor_item",
                () -> hex(session, ClientboundSetCursorItemPacket.STREAM_CODEC, new ClientboundSetCursorItemPacket(stack(session, "stone", 64)))
            );
            labels.put(
                "set_player_inventory",
                () -> hex(session, ClientboundSetPlayerInventoryPacket.STREAM_CODEC, new ClientboundSetPlayerInventoryPacket(36, stack(session, "apple", 3)))
            );
            labels.put(
                "open_screen",
                () -> hex(session, ClientboundOpenScreenPacket.STREAM_CODEC, new ClientboundOpenScreenPacket(1, MenuType.GENERIC_9x3, Component.literal("Chest")))
            );
            labels.put("container_close", () -> hex(session, ClientboundContainerClosePacket.STREAM_CODEC, new ClientboundContainerClosePacket(2)));
            labels.put(
                "container_set_data",
                () -> hex(session, ClientboundContainerSetDataPacket.STREAM_CODEC, new ClientboundContainerSetDataPacket(1, 2, 300))
            );
            labels.put("set_held_slot", () -> hex(session, ClientboundSetHeldSlotPacket.STREAM_CODEC, new ClientboundSetHeldSlotPacket(4)));
            labels.put(
                "take_item_entity",
                () -> hex(session, ClientboundTakeItemEntityPacket.STREAM_CODEC, new ClientboundTakeItemEntityPacket(10, 20, 3))
            );
            labels.put("merchant_offers", () -> {
                MerchantOffers offers = new MerchantOffers();
                offers.addAll(merchantOffers(session));
                return hex(session, ClientboundMerchantOffersPacket.STREAM_CODEC, new ClientboundMerchantOffersPacket(5, offers, 2, 15, true, false));
            });
            labels.put(
                "merchant_offers_empty",
                () -> hex(
                    session, ClientboundMerchantOffersPacket.STREAM_CODEC, new ClientboundMerchantOffersPacket(1, new MerchantOffers(), 1, 0, false, true)
                )
            );
            labels.put(
                "set_creative_mode_slot",
                () -> hex(session, ServerboundSetCreativeModeSlotPacket.STREAM_CODEC, new ServerboundSetCreativeModeSlotPacket(36, sword(session)))
            );
            labels.put(
                "set_creative_mode_slot_empty",
                () -> hex(session, ServerboundSetCreativeModeSlotPacket.STREAM_CODEC, new ServerboundSetCreativeModeSlotPacket(-1, ItemStack.EMPTY))
            );
            labels.put("sb_container_close", () -> hex(session, ServerboundContainerClosePacket.STREAM_CODEC, new ServerboundContainerClosePacket(3)));
            labels.put(
                "container_button_click",
                () -> hex(session, ServerboundContainerButtonClickPacket.STREAM_CODEC, new ServerboundContainerButtonClickPacket(1, 2))
            );
            labels.put(
                "container_slot_state_changed",
                () -> hex(session, ServerboundContainerSlotStateChangedPacket.STREAM_CODEC, new ServerboundContainerSlotStateChangedPacket(5, 1, true))
            );
            labels.put("rename_item", () -> hex(session, ServerboundRenameItemPacket.STREAM_CODEC, new ServerboundRenameItemPacket("Excalibur")));
            labels.put("select_trade", () -> hex(session, ServerboundSelectTradePacket.STREAM_CODEC, new ServerboundSelectTradePacket(1)));
            labels.put(
                "edit_book",
                () -> hex(
                    session, ServerboundEditBookPacket.STREAM_CODEC, new ServerboundEditBookPacket(0, List.of("page one", "page two"), Optional.of("Title"))
                )
            );
            labels.put(
                "edit_book_untitled",
                () -> hex(session, ServerboundEditBookPacket.STREAM_CODEC, new ServerboundEditBookPacket(1, List.of(), Optional.empty()))
            );
            labels.put(
                "pick_item_from_block",
                () -> hex(session, ServerboundPickItemFromBlockPacket.STREAM_CODEC, new ServerboundPickItemFromBlockPacket(new BlockPos(1, -2, 3), true))
            );
            labels.put(
                "pick_item_from_entity",
                () -> hex(session, ServerboundPickItemFromEntityPacket.STREAM_CODEC, new ServerboundPickItemFromEntityPacket(42, false))
            );
            labels.put("container_click", () -> hex(session, ServerboundContainerClickPacket.STREAM_CODEC, containerClick(session)));
            rewriteLabelled(session, current, output, labels);
        });
    }

    static void joinPackets(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            Map<String, Supplier<String>> labels = new LinkedHashMap<>();
            labels.put(
                "login",
                () -> hex(
                    session,
                    ClientboundLoginPacket.STREAM_CODEC,
                    new ClientboundLoginPacket(
                        7,
                        false,
                        Set.of(dimension("overworld")),
                        20,
                        10,
                        8,
                        false,
                        true,
                        false,
                        new CommonPlayerSpawnInfo(
                            dimensionType(session, "overworld"),
                            dimension("overworld"),
                            GameType.CREATIVE,
                            Optional.of(GameType.SURVIVAL),
                            false,
                            false,
                            Optional.empty(),
                            5,
                            63
                        ),
                        false,
                        true
                    )
                )
            );
            labels.put(
                "respawn",
                () -> hex(
                    session,
                    ClientboundRespawnPacket.STREAM_CODEC,
                    new ClientboundRespawnPacket(
                        new CommonPlayerSpawnInfo(
                            dimensionType(session, "the_nether"),
                            dimension("the_nether"),
                            GameType.SURVIVAL,
                            Optional.empty(),
                            false,
                            true,
                            Optional.of(GlobalPos.of(dimension("overworld"), new BlockPos(1, 64, -3))),
                            0,
                            32
                        ),
                        ClientboundRespawnPacket.KEEP_ALL_DATA
                    )
                )
            );
            labels.put(
                "intention",
                () -> hex(
                    session,
                    ClientIntentionPacket.STREAM_CODEC,
                    new ClientIntentionPacket(SharedConstants.getProtocolVersion(), "example.org", 25565, ClientIntent.LOGIN)
                )
            );
            labels.put(
                "intention_host_at_bound",
                () -> hex(
                    session,
                    ClientIntentionPacket.STREAM_CODEC,
                    new ClientIntentionPacket(SharedConstants.getProtocolVersion(), "a".repeat(1024), 25565, ClientIntent.STATUS)
                )
            );
            rewriteLabelled(session, current, output, labels);
        });
    }

    private static Holder<DimensionType> dimensionType(final Session session, final String path) {
        return session.access()
            .lookupOrThrow(Registries.DIMENSION_TYPE)
            .getOrThrow(ResourceKey.create(Registries.DIMENSION_TYPE, Identifier.withDefaultNamespace(path)));
    }

    private static ResourceKey<Level> dimension(final String path) {
        return ResourceKey.create(Registries.DIMENSION, Identifier.withDefaultNamespace(path));
    }

    static void particles(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            List<String> in = Files.readAllLines(current, StandardCharsets.UTF_8);
            Map<String, String> old = new LinkedHashMap<>();
            for (String line : in) {
                int separator = line.indexOf(" = ");
                if (separator > 0 && !line.startsWith("type ")) {
                    old.put(line.substring(0, separator), line.substring(separator + 3));
                }
            }
            List<String> out = new ArrayList<>();
            boolean typesWritten = false;
            for (String line : in) {
                if (line.startsWith("type ")) {
                    if (!typesWritten) {
                        for (ParticleType<?> type : BuiltInRegistries.PARTICLE_TYPE) {
                            out.add(
                                "type " + BuiltInRegistries.PARTICLE_TYPE.getId(type) + " " + BuiltInRegistries.PARTICLE_TYPE.getKey(type)
                                    + " limiter=" + type.getOverrideLimiter()
                            );
                        }
                        typesWritten = true;
                    }
                    continue;
                }
                int separator = line.indexOf(" = ");
                if (separator < 0) {
                    throw new IllegalStateException("malformed line: " + line);
                }
                String key = line.substring(0, separator);
                out.add(key + " = " + particleValue(session, key, old));
            }
            if (!typesWritten) {
                throw new IllegalStateException("the file has no type lines");
            }
            CodecGoldens.writeLines(output, out);
        });
    }

    private static String particleValue(final Session session, final String key, final Map<String, String> old) {
        switch (key) {
            case "furnace_lit_id":
                return Integer.toString(Block.BLOCK_STATE_REGISTRY.getId(furnace(Direction.NORTH, true)));
            case "furnace_default_id":
                return Integer.toString(Block.BLOCK_STATE_REGISTRY.getId(Blocks.FURNACE.defaultBlockState()));
            case "furnace_south_id":
                return Integer.toString(Block.BLOCK_STATE_REGISTRY.getId(furnace(Direction.SOUTH, false)));
            case "stone_id":
                return Integer.toString(Block.BLOCK_STATE_REGISTRY.getId(Blocks.STONE.defaultBlockState()));
            case "item_stone_id":
                return Integer.toString(itemId(session, "stone"));
            case "item_apple_id":
                return Integer.toString(itemId(session, "apple"));
            case "item_diamond_sword_id":
                return Integer.toString(itemId(session, "diamond_sword"));
            default:
                break;
        }
        int dot = key.indexOf('.');
        if (dot < 0) {
            throw new IllegalStateException("no inputs for " + key);
        }
        String label = key.substring(0, dot);
        return switch (key.substring(dot + 1)) {
            case "wire" -> labelWire(session, label, old);
            case "json" -> labelJson(session, label, old);
            case "input" -> old.get(key);
            case "parsed_wire" -> hex(session, ParticleTypes.STREAM_CODEC, parsed(session, label, old));
            case "parsed_json" -> json(session, ParticleTypes.CODEC, parsed(session, label, old));
            case "parse_error" -> parseError(session, label, old);
            default -> throw new IllegalStateException("no inputs for " + key);
        };
    }

    private static String labelWire(final Session session, final String label, final Map<String, String> old) {
        return switch (label) {
            case "particles_packet" -> hex(
                session,
                ClientboundLevelParticlesPacket.STREAM_CODEC,
                new ClientboundLevelParticlesPacket(options(session, "dust", old), true, false, 1.5, 64.25, -3.0, 0.5f, 0.75f, 1.0f, 0.1f, 25)
            );
            case "particles_packet_item" -> hex(
                session,
                ClientboundLevelParticlesPacket.STREAM_CODEC,
                new ClientboundLevelParticlesPacket(options(session, "item", old), false, true, 0.0, 0.0, 0.0, 0.0f, 0.0f, 0.0f, 0.0f, 3)
            );
            case "root" -> hex(session, Advancement.STREAM_CODEC, rootAdvancement(session));
            case "advancements" -> hex(session, ClientboundUpdateAdvancementsPacket.STREAM_CODEC, updateAdvancements(session));
            case "advancements_empty" -> hex(
                session,
                ClientboundUpdateAdvancementsPacket.STREAM_CODEC,
                new ClientboundUpdateAdvancementsPacket(false, List.of(), Set.of(), Map.of(), false)
            );
            case "seen_opened" -> hex(
                session,
                ServerboundSeenAdvancementsPacket.STREAM_CODEC,
                new ServerboundSeenAdvancementsPacket(ServerboundSeenAdvancementsPacket.Action.OPENED_TAB, Identifier.parse("mcrs:root"))
            );
            case "seen_closed" -> hex(
                session,
                ServerboundSeenAdvancementsPacket.STREAM_CODEC,
                new ServerboundSeenAdvancementsPacket(ServerboundSeenAdvancementsPacket.Action.CLOSED_SCREEN, null)
            );
            default -> hex(session, ParticleTypes.STREAM_CODEC, options(session, label, old));
        };
    }

    private static String labelJson(final Session session, final String label, final Map<String, String> old) {
        return switch (label) {
            case "root_display" -> json(session, DisplayInfo.CODEC, rootDisplay(session, true));
            case "child_display" -> json(session, DisplayInfo.CODEC, childDisplay(session));
            default -> json(session, ParticleTypes.CODEC, options(session, label, old));
        };
    }

    private static ParticleOptions options(final Session session, final String label, final Map<String, String> old) {
        String input = old.get(label + ".json");
        if (input == null) {
            throw new IllegalStateException("no json line for label " + label);
        }
        return ParticleTypes.CODEC.parse(session.json(), JsonParser.parseString(input)).getOrThrow(IllegalStateException::new);
    }

    private static ParticleOptions parsed(final Session session, final String label, final Map<String, String> old) {
        DataResult<ParticleOptions> result = parse(session, label, old);
        return result.result().orElseThrow(() -> new IllegalStateException(label + " no longer parses"));
    }

    private static String parseError(final Session session, final String label, final Map<String, String> old) {
        DataResult<ParticleOptions> result = parse(session, label, old);
        return result.error().orElseThrow(() -> new IllegalStateException(label + " no longer fails to parse")).message();
    }

    private static DataResult<ParticleOptions> parse(final Session session, final String label, final Map<String, String> old) {
        String input = PARSE_ERROR_INPUTS.containsKey(label) ? PARSE_ERROR_INPUTS.get(label) : old.get(label + ".input");
        if (input == null) {
            throw new IllegalStateException("no input for label " + label);
        }
        return ParticleTypes.CODEC.parse(session.json(), JsonParser.parseString(input));
    }

    private static BlockState furnace(final Direction facing, final boolean lit) {
        return Blocks.FURNACE.defaultBlockState().setValue(AbstractFurnaceBlock.FACING, facing).setValue(AbstractFurnaceBlock.LIT, lit);
    }

    private static int itemId(final Session session, final String path) {
        return BuiltInRegistries.ITEM.getId(item(session, path).value());
    }

    private static DisplayInfo rootDisplay(final Session session, final boolean announceToChat) {
        return new DisplayInfo(
            new ItemStackTemplate(item(session, "apple"), 1, DataComponentPatch.EMPTY),
            Component.literal("Root"),
            Component.translatable("adv.desc"),
            Optional.of(new ClientAsset.ResourceTexture(Identifier.parse("minecraft:gui/advancements/backgrounds/stone"))),
            AdvancementType.CHALLENGE,
            true,
            announceToChat,
            false
        );
    }

    private static DisplayInfo childDisplay(final Session session) {
        return new DisplayInfo(
            new ItemStackTemplate(item(session, "diamond_sword"), 2, DataComponentPatch.builder().set(DataComponents.MAX_STACK_SIZE, 16).build()),
            Component.literal("Child"),
            Component.literal("hidden one"),
            Optional.empty(),
            AdvancementType.GOAL,
            false,
            false,
            true
        );
    }

    private static Advancement rootAdvancement(final Session session) {
        return new Advancement(
            Optional.empty(),
            Optional.of(rootDisplay(session, false)),
            AdvancementRewards.EMPTY,
            Map.of(),
            new AdvancementRequirements(List.of(List.of("a", "b"), List.of("c"))),
            true
        );
    }

    private static ClientboundUpdateAdvancementsPacket updateAdvancements(final Session session) {
        Advancement child = new Advancement(
            Optional.of(Identifier.parse("mcrs:root")),
            Optional.of(childDisplay(session)),
            AdvancementRewards.EMPTY,
            Map.of(),
            new AdvancementRequirements(List.of()),
            false
        );
        Advancement bare = new Advancement(
            Optional.of(Identifier.parse("mcrs:root")),
            Optional.empty(),
            AdvancementRewards.EMPTY,
            Map.of(),
            new AdvancementRequirements(List.of(List.of("x"))),
            false
        );
        Set<Identifier> removed = new LinkedHashSet<>();
        removed.add(Identifier.parse("minecraft:story/root"));
        removed.add(Identifier.parse("mcrs:gone"));
        Map<String, CriterionProgress> criteria = new LinkedHashMap<>();
        criteria.put("a", new CriterionProgress(Instant.ofEpochMilli(1700000000123L)));
        criteria.put("b", new CriterionProgress());
        Map<Identifier, AdvancementProgress> progress = new LinkedHashMap<>();
        progress.put(Identifier.parse("mcrs:root"), progress(criteria));
        progress.put(Identifier.parse("mcrs:child"), new AdvancementProgress());
        return new ClientboundUpdateAdvancementsPacket(
            true,
            List.of(
                new ClientboundUpdateAdvancementsPacket.PositionedAdvancement(new AdvancementHolder(Identifier.parse("mcrs:root"), rootAdvancement(session)), 0.5f, 1.5f),
                new ClientboundUpdateAdvancementsPacket.PositionedAdvancement(new AdvancementHolder(Identifier.parse("mcrs:child"), child), 2.0f, -1.0f),
                new ClientboundUpdateAdvancementsPacket.PositionedAdvancement(new AdvancementHolder(Identifier.parse("mcrs:bare"), bare), 0.0f, 0.0f)
            ),
            removed,
            progress,
            true
        );
    }

    // an obtained time can only be set through the wire form: the field is private and grant() reads the clock
    private static AdvancementProgress progress(final Map<String, CriterionProgress> criteria) {
        ByteBuf buffer = Unpooled.buffer();
        ByteBufCodecs.<ByteBuf, String, CriterionProgress, Map<String, CriterionProgress>>map(
            LinkedHashMap::new, ByteBufCodecs.STRING_UTF8, CriterionProgress.STREAM_CODEC
        ).encode(buffer, criteria);
        return AdvancementProgress.STREAM_CODEC.decode(buffer);
    }

    private static ClientboundRecipeBookAddPacket recipeBookAdd(final Session session) {
        Holder<Item> stone = item(session, "stone");
        HolderSet<Item> planks = session.access().lookupOrThrow(Registries.ITEM).getOrThrow(ItemTags.PLANKS);
        ItemStackTemplate sword = new ItemStackTemplate(
            item(session, "diamond_sword"), 2, DataComponentPatch.builder().set(DataComponents.DAMAGE, 3).build()
        );
        Holder<TrimPattern> coast = session.access().lookupOrThrow(Registries.TRIM_PATTERN).getOrThrow(
            ResourceKey.create(Registries.TRIM_PATTERN, Identifier.withDefaultNamespace("coast"))
        );
        Holder<TrimPattern> wave = Holder.direct(new TrimPattern(Identifier.parse("mcrs:wave"), Component.literal("Wave"), true));
        return new ClientboundRecipeBookAddPacket(
            List.of(
                entry(
                    0,
                    new ShapelessCraftingRecipeDisplay(
                        List.of(
                            slot(session, "stone"),
                            new SlotDisplay.TagSlotDisplay(planks),
                            new SlotDisplay.TagSlotDisplay(HolderSet.direct(stone, item(session, "apple")))
                        ),
                        new SlotDisplay.ItemStackSlotDisplay(sword),
                        slot(session, "crafting_table")
                    ),
                    OptionalInt.of(5),
                    RecipeBookCategories.CRAFTING_MISC,
                    Optional.of(List.of(Ingredient.of(HolderSet.direct(stone)), Ingredient.of(planks))),
                    ClientboundRecipeBookAddPacket.Entry.FLAG_NOTIFICATION | ClientboundRecipeBookAddPacket.Entry.FLAG_HIGHLIGHT
                ),
                entry(
                    1,
                    new ShapedCraftingRecipeDisplay(
                        2,
                        1,
                        List.of(SlotDisplay.Empty.INSTANCE, SlotDisplay.AnyFuel.INSTANCE),
                        new SlotDisplay.WithRemainder(slot(session, "water_bucket"), slot(session, "bucket")),
                        new SlotDisplay.Composite(List.of(slot(session, "crafting_table"), slot(session, "stone")))
                    ),
                    OptionalInt.empty(),
                    RecipeBookCategories.CRAFTING_BUILDING_BLOCKS,
                    Optional.empty(),
                    0
                ),
                entry(
                    2,
                    new FurnaceRecipeDisplay(
                        new SlotDisplay.WithAnyPotion(slot(session, "potion")),
                        SlotDisplay.AnyFuel.INSTANCE,
                        new SlotDisplay.OnlyWithComponent(slot(session, "diamond_sword"), DataComponents.DAMAGE),
                        slot(session, "furnace"),
                        200,
                        0.35f
                    ),
                    OptionalInt.of(0),
                    RecipeBookCategories.FURNACE_FOOD,
                    Optional.of(List.of()),
                    ClientboundRecipeBookAddPacket.Entry.FLAG_NOTIFICATION
                ),
                entry(
                    300,
                    new StonecutterRecipeDisplay(
                        new SlotDisplay.DyedSlotDemo(slot(session, "red_dye"), slot(session, "leather_helmet")),
                        new SlotDisplay.SmithingTrimDemoSlotDisplay(slot(session, "iron_chestplate"), slot(session, "stone"), coast),
                        SlotDisplay.Empty.INSTANCE
                    ),
                    OptionalInt.empty(),
                    RecipeBookCategories.STONECUTTER,
                    Optional.empty(),
                    ClientboundRecipeBookAddPacket.Entry.FLAG_HIGHLIGHT
                ),
                entry(
                    4,
                    new SmithingRecipeDisplay(
                        slot(session, "netherite_upgrade_smithing_template"),
                        slot(session, "iron_chestplate"),
                        slot(session, "netherite_ingot"),
                        new SlotDisplay.SmithingTrimDemoSlotDisplay(slot(session, "iron_chestplate"), slot(session, "netherite_ingot"), wave),
                        slot(session, "smithing_table")
                    ),
                    OptionalInt.empty(),
                    RecipeBookCategories.SMITHING,
                    Optional.empty(),
                    0
                )
            ),
            true
        );
    }

    private static ClientboundRecipeBookAddPacket.Entry entry(
        final int id,
        final RecipeDisplay display,
        final OptionalInt group,
        final RecipeBookCategory category,
        final Optional<List<Ingredient>> craftingRequirements,
        final int flags
    ) {
        return new ClientboundRecipeBookAddPacket.Entry(
            new RecipeDisplayEntry(new RecipeDisplayId(id), display, group, category, craftingRequirements), (byte) flags
        );
    }

    private static ClientboundUpdateRecipesPacket updateRecipes(final Session session) {
        Map<ResourceKey<RecipePropertySet>, RecipePropertySet> itemSets = new LinkedHashMap<>();
        itemSets.put(
            RecipePropertySet.SMITHING_BASE,
            RecipePropertySet.create(List.of(Ingredient.of(HolderSet.direct(item(session, "iron_chestplate")))))
        );
        itemSets.put(RecipePropertySet.FURNACE_INPUT, RecipePropertySet.create(List.of()));
        HolderSet<Item> planks = session.access().lookupOrThrow(Registries.ITEM).getOrThrow(ItemTags.PLANKS);
        return new ClientboundUpdateRecipesPacket(
            itemSets,
            new SelectableRecipe.SingleInputSet<StonecutterRecipe>(
                List.of(
                    new SelectableRecipe.SingleInputEntry<StonecutterRecipe>(
                        Ingredient.of(HolderSet.direct(item(session, "stone"))),
                        new SelectableRecipe<StonecutterRecipe>(
                            new SlotDisplay.ItemStackSlotDisplay(new ItemStackTemplate(item(session, "stone_bricks"), 4, DataComponentPatch.EMPTY)),
                            Optional.empty()
                        )
                    ),
                    new SelectableRecipe.SingleInputEntry<StonecutterRecipe>(
                        Ingredient.of(planks), new SelectableRecipe<StonecutterRecipe>(slot(session, "oak_planks"), Optional.empty())
                    )
                )
            )
        );
    }

    private static RecipeBookSettings recipeBookSettings() {
        RecipeBookSettings settings = new RecipeBookSettings();
        settings.setOpen(RecipeBookType.CRAFTING, true);
        settings.setFiltering(RecipeBookType.FURNACE, true);
        settings.setOpen(RecipeBookType.SMOKER, true);
        settings.setFiltering(RecipeBookType.SMOKER, true);
        return settings;
    }

    private static List<MerchantOffer> merchantOffers(final Session session) {
        MerchantOffer first = new MerchantOffer(
            new ItemCost(item(session, "emerald"), 3, DataComponentExactPredicate.EMPTY),
            Optional.empty(),
            stack(session, "apple", 2),
            1,
            12,
            5,
            0.05f,
            2
        );
        first.setSpecialPriceDiff(-1);
        ItemStack soldSword = stack(session, "diamond_sword", 1);
        soldSword.set(DataComponents.DAMAGE, 3);
        MerchantOffer second = new MerchantOffer(
            new ItemCost(
                item(session, "diamond"),
                1,
                DataComponentExactPredicate.builder()
                    .expect(DataComponents.MAX_STACK_SIZE, 16)
                    .expect(DataComponents.CUSTOM_NAME, Component.literal("x"))
                    .build()
            ),
            Optional.of(new ItemCost(item(session, "stone"), 4, DataComponentExactPredicate.EMPTY)),
            soldSword,
            4,
            4,
            0,
            0.2f,
            0
        );
        return List.of(first, second);
    }

    // vanilla builds this map as an identity hash map, so its entry order differs between runs; it is written in registry id order
    private static ServerboundContainerClickPacket containerClick(final Session session) {
        ItemStack sword = sword(session);
        DataComponentPatch.SplitResult split = sword.getComponentsPatch().split();
        Map<DataComponentType<?>, Integer> added = new LinkedHashMap<>();
        split.added().stream()
            .sorted(Comparator.comparingInt(component -> BuiltInRegistries.DATA_COMPONENT_TYPE.getId(component.type())))
            .forEach(component -> added.put(component.type(), hash(session, component)));
        Set<DataComponentType<?>> removed = new LinkedHashSet<>();
        split.removed().stream().sorted(Comparator.comparingInt(BuiltInRegistries.DATA_COMPONENT_TYPE::getId)).forEach(removed::add);
        Int2ObjectMap<HashedStack> changed = new Int2ObjectOpenHashMap<>();
        changed.put(4, HashedStack.EMPTY);
        changed.put(3, new HashedStack.ActualItem(sword.typeHolder(), sword.getCount(), new HashedPatchMap(added, removed)));
        ItemStack apple = stack(session, "apple", 1);
        HashedStack carried = new HashedStack.ActualItem(apple.typeHolder(), 1, new HashedPatchMap(Map.of(), Set.of()));
        return new ServerboundContainerClickPacket(1, 2, (short) 3, (byte) 0, ContainerInput.PICKUP, changed, carried);
    }

    private static <T> int hash(final Session session, final TypedDataComponent<T> component) {
        return component.encodeValue(session.hash()).getOrThrow(IllegalStateException::new).asInt();
    }

    private static ItemStack sword(final Session session) {
        ItemStack sword = stack(session, "diamond_sword", 1);
        sword.set(DataComponents.DAMAGE, 7);
        sword.set(DataComponents.CUSTOM_NAME, Component.literal("named"));
        sword.set(DataComponents.UNBREAKABLE, Unit.INSTANCE);
        return sword;
    }

    private static ItemStack stack(final Session session, final String path, final int count) {
        return new ItemStack(item(session, path), count);
    }

    private static List<ItemStack> emptyStacks(final int count) {
        List<ItemStack> stacks = new ArrayList<>();
        for (int index = 0; index < count; index++) {
            stacks.add(ItemStack.EMPTY);
        }
        return stacks;
    }

    private static SlotDisplay slot(final Session session, final String path) {
        return new SlotDisplay.ItemSlotDisplay(item(session, path));
    }

    private static Holder<Item> item(final Session session, final String path) {
        return session.access().lookupOrThrow(Registries.ITEM).getOrThrow(ResourceKey.create(Registries.ITEM, Identifier.withDefaultNamespace(path)));
    }

    private static <T> String json(final Session session, final com.mojang.serialization.Codec<T> codec, final T value) {
        return CodecGoldens.GSON.toJson(codec.encodeStart(session.json(), value).getOrThrow(IllegalStateException::new));
    }
}
